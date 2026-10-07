//! Atomic mutations are restricted to the dedicated application table.
use serde::Serialize;
use serde_json::Value;

use crate::{
    command::Runner,
    config::TorConfig,
    error::{AnonError, Result},
};

const OWNER: &str = "omatorsurf managed table v1";

#[derive(Debug, Serialize)]
pub struct FirewallStatus {
    pub table_present: bool,
    pub managed: bool,
}

pub fn tor_uid(runner: &impl Runner) -> Result<u32> {
    let output = runner
        .run("getent", &["passwd", "tor"])?
        .checked("getent")?;
    let fields: Vec<_> = output.trim().split(':').collect();
    if fields.first() != Some(&"tor") {
        return Err(AnonError::Firewall(
            "getent did not return the tor account".into(),
        ));
    }
    let uid = fields
        .get(2)
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value != 0)
        .ok_or_else(|| {
            AnonError::Firewall("the tor account must have a valid non-root UID".into())
        })?;
    Ok(uid)
}

pub fn render(config: &TorConfig, uid: u32) -> Result<String> {
    if uid == 0 {
        return Err(AnonError::Firewall(
            "refusing to exempt root from Tor redirection".into(),
        ));
    }
    Ok(include_str!("../nftables/omatorsurf.nft")
        .replace("@TOR_UID@", &uid.to_string())
        .replace("@DNS_PORT@", &config.dns_port.to_string())
        .replace("@TRANS_PORT@", &config.trans_port.to_string()))
}

fn nft_items(output: &str) -> Result<Vec<Value>> {
    let parsed: Value = serde_json::from_str(output)
        .map_err(|error| AnonError::Firewall(format!("invalid nft JSON: {error}")))?;
    parsed
        .get("nftables")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AnonError::Firewall("nft JSON has no nftables array".into()))
}

pub struct Firewall<'a, R> {
    config: &'a TorConfig,
    runner: &'a R,
}

impl<'a, R: Runner> Firewall<'a, R> {
    pub fn new(config: &'a TorConfig, runner: &'a R) -> Self {
        Self { config, runner }
    }

    /// Compare every installed chain and ordered rule, not just ownership.
    /// Unexpected native formatting is conservatively treated as unverified.
    pub fn verified(&self) -> Result<bool> {
        let state = self.status()?;
        if !state.managed {
            return Ok(false);
        }
        let actual = self
            .runner
            .run(
                "nft",
                &[
                    "--stateless",
                    "--numeric",
                    "list",
                    "table",
                    "inet",
                    "omatorsurf",
                ],
            )?
            .checked("nft")?;
        Ok(matches_expected(
            &actual,
            self.config,
            tor_uid(self.runner)?,
        ))
    }

    pub fn status(&self) -> Result<FirewallStatus> {
        let output = self
            .runner
            .run("nft", &["--json", "list", "tables"])?
            .checked("nft")?;
        let present = nft_items(&output)?.iter().any(|item| {
            item.get("table")
                .is_some_and(|table| table["family"] == "inet" && table["name"] == "omatorsurf")
        });
        if !present {
            return Ok(FirewallStatus {
                table_present: false,
                managed: false,
            });
        }
        let output = self
            .runner
            .run("nft", &["--json", "list", "table", "inet", "omatorsurf"])?
            .checked("nft")?;
        let managed = nft_items(&output)?.iter().any(|item| {
            item.get("table").is_some_and(|table| {
                table["family"] == "inet"
                    && table["name"] == "omatorsurf"
                    && table["comment"] == OWNER
            })
        });
        Ok(FirewallStatus {
            table_present: true,
            managed,
        })
    }

    fn batch(&self) -> Result<String> {
        let state = self.status()?;
        if state.table_present && !state.managed {
            return Err(AnonError::Firewall("inet omatorsurf exists without this application's ownership marker; refusing to change it".into()));
        }
        let rules = render(self.config, tor_uid(self.runner)?)?;
        Ok(if state.table_present {
            format!("delete table inet omatorsurf\n{rules}")
        } else {
            rules
        })
    }

    pub fn check(&self) -> Result<()> {
        let batch = self.batch()?;
        self.runner
            .run_input("nft", &["--check", "--file", "-"], &batch)?
            .checked("nft")?;
        Ok(())
    }

    /// Called after privilege checks; safe before Tor readiness for fail-closed startup.
    pub fn apply(&self) -> Result<()> {
        let batch = self.batch()?;
        // Validate first, then submit delete/create as one kernel transaction.
        self.runner
            .run_input("nft", &["--check", "--file", "-"], &batch)?
            .checked("nft")?;
        self.runner
            .run_input("nft", &["--file", "-"], &batch)?
            .checked("nft")?;
        if !self.status()?.managed {
            return Err(AnonError::Firewall(
                "application table not observed after apply; any installed rules remain in place"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Intentional disable operation; never called automatically after Tor failure.
    pub fn remove(&self) -> Result<()> {
        let state = self.status()?;
        if !state.table_present {
            return Ok(());
        }
        if !state.managed {
            return Err(AnonError::Firewall(
                "refusing to remove an unrecognized inet omatorsurf table".into(),
            ));
        }
        self.runner
            .run("nft", &["delete", "table", "inet", "omatorsurf"])?
            .checked("nft")?;
        if self.status()?.table_present {
            return Err(AnonError::Firewall(
                "inet omatorsurf still exists after removal".into(),
            ));
        }
        Ok(())
    }
}

fn matches_expected(actual: &str, config: &TorConfig, uid: u32) -> bool {
    let expected = include_str!("../nftables/expected.nft")
        .replace("@TOR_UID@", &uid.to_string())
        .replace("@DNS_PORT@", &config.dns_port.to_string())
        .replace("@TRANS_PORT@", &config.trans_port.to_string());
    actual.split_whitespace().eq(expected.split_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandOutput;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    struct Fake {
        outputs: RefCell<VecDeque<(i32, String)>>,
        calls: RefCell<Vec<(String, Vec<String>, String)>>,
    }
    impl Fake {
        fn new(outputs: &[(i32, &str)]) -> Self {
            Self {
                outputs: RefCell::new(
                    outputs
                        .iter()
                        .map(|(code, text)| (*code, (*text).into()))
                        .collect(),
                ),
                calls: RefCell::new(vec![]),
            }
        }
        fn next(&self, program: &str, args: &[&str], input: &str) -> Result<CommandOutput> {
            self.calls.borrow_mut().push((
                program.into(),
                args.iter().map(|arg| (*arg).into()).collect(),
                input.into(),
            ));
            let (code, stdout) = self
                .outputs
                .borrow_mut()
                .pop_front()
                .expect("unexpected command");
            Ok(CommandOutput {
                code: Some(code),
                stdout,
                stderr: "mock failure".into(),
            })
        }
    }
    impl Runner for Fake {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput> {
            self.next(program, args, "")
        }
        fn run_input(&self, program: &str, args: &[&str], input: &str) -> Result<CommandOutput> {
            self.next(program, args, input)
        }
    }
    const EMPTY: &str = r#"{"nftables":[]}"#;
    const PRESENT: &str = r#"{"nftables":[{"table":{"family":"inet","name":"omatorsurf","comment":"omatorsurf managed table v1"}}]}"#;
    const OTHER: &str = r#"{"nftables":[{"table":{"family":"inet","name":"other"}},{"table":{"family":"ip","name":"omatorsurf"}}]}"#;
    const ACCOUNT: &str = "tor:x:43:43::/var/lib/tor:/usr/bin/nologin";

    #[test]
    fn protection_requires_all_rules_and_exact_policies() {
        let config = TorConfig::default();
        let expected = include_str!("../nftables/expected.nft")
            .replace("@TOR_UID@", "43")
            .replace("@DNS_PORT@", "5353")
            .replace("@TRANS_PORT@", "9040");
        assert!(matches_expected(&expected, &config, 43));
        for bad in [
            expected.replace("policy drop", "policy accept"),
            expected.replace("udp dport 53", "udp dport 54"),
            expected.replace("meta skuid 43", "meta skuid 0"),
            expected.replace("oifname \"lo\" accept", "accept"),
            expected.replace("ct status 0x20", "ct state established"),
            expected.replace("priority 0", "priority 1"),
            expected.replace("guard_forward", "other"),
        ] {
            assert!(!matches_expected(&bad, &config, 43));
        }
        assert!(!matches_expected(&expected, &config, 44));
    }

    #[test]
    fn template_routes_dns_before_loopback_and_exempts_tor_first() {
        let config = TorConfig {
            dns_port: 5453,
            trans_port: 9140,
            ..TorConfig::default()
        };
        let text = render(&config, 99).unwrap();
        assert!(text.contains("redirect to :5453"));
        assert!(text.contains("redirect to :9140"));
        assert!(text.contains("ip daddr 127.0.0.1 ct status dnat tcp dport 9140 accept"));
        assert!(text.contains("ip daddr 127.0.0.1 ct status dnat udp dport 5453 accept"));
        assert!(text.find("meta skuid 99 return").unwrap() < text.find("udp dport 53").unwrap());
        assert!(text.find("udp dport 53").unwrap() < text.find("oifname \"lo\" return").unwrap());
        assert!(!text.contains('@'));
        assert!(!text.contains("flush ruleset"));
        assert!(!text.contains("ct state established"));
        assert!(render(&config, 0).is_err());
    }

    #[test]
    fn observation_errors_are_not_treated_as_missing_table() {
        for (code, json) in [(1, ""), (0, "garbage"), (0, "{}")] {
            let runner = Fake::new(&[(code, json)]);
            assert!(
                Firewall::new(&TorConfig::default(), &runner)
                    .status()
                    .is_err()
            );
        }
    }

    #[test]
    fn unrelated_tables_do_not_count_as_application_table() {
        let runner = Fake::new(&[(0, OTHER)]);
        assert!(
            !Firewall::new(&TorConfig::default(), &runner)
                .status()
                .unwrap()
                .table_present
        );
    }

    #[test]
    fn absent_table_removal_is_idempotent() {
        let runner = Fake::new(&[(0, EMPTY)]);
        Firewall::new(&TorConfig::default(), &runner)
            .remove()
            .unwrap();
        assert_eq!(runner.calls.borrow().len(), 1);
    }

    #[test]
    fn unrecognized_table_cannot_be_removed_or_replaced() {
        let unknown = r#"{"nftables":[{"table":{"family":"inet","name":"omatorsurf"}}]}"#;
        for remove in [true, false] {
            let runner = Fake::new(&[(0, PRESENT), (0, unknown)]);
            let config = TorConfig::default();
            let firewall = Firewall::new(&config, &runner);
            assert!(
                if remove {
                    firewall.remove()
                } else {
                    firewall.apply()
                }
                .is_err()
            );
            assert_eq!(runner.calls.borrow().len(), 2);
        }
    }

    #[test]
    fn existing_table_is_replaced_atomically_without_touching_other_tables() {
        let runner = Fake::new(&[
            (0, PRESENT),
            (0, PRESENT),
            (0, ACCOUNT),
            (0, ""),
            (0, ""),
            (0, PRESENT),
            (0, PRESENT),
        ]);
        Firewall::new(&TorConfig::default(), &runner)
            .apply()
            .unwrap();
        let calls = runner.calls.borrow();
        assert_eq!(calls[3].1, ["--check", "--file", "-"]);
        assert_eq!(calls[4].1, ["--file", "-"]);
        assert_eq!(calls[3].2, calls[4].2);
        assert!(calls[4].2.starts_with("delete table inet omatorsurf\n"));
        assert_eq!(calls[4].2.matches("table inet omatorsurf {").count(), 1);
    }

    #[test]
    fn validation_failure_never_submits_mutation() {
        let runner = Fake::new(&[(0, EMPTY), (0, ACCOUNT), (1, "")]);
        assert!(
            Firewall::new(&TorConfig::default(), &runner)
                .apply()
                .is_err()
        );
        assert_eq!(runner.calls.borrow().len(), 3);
    }

    #[test]
    fn failed_apply_does_not_remove_protection() {
        let runner = Fake::new(&[(0, EMPTY), (0, ACCOUNT), (0, ""), (1, "")]);
        assert!(
            Firewall::new(&TorConfig::default(), &runner)
                .apply()
                .is_err()
        );
        assert_eq!(runner.calls.borrow().len(), 4);
    }

    #[test]
    fn removal_targets_only_owned_table() {
        let runner = Fake::new(&[(0, PRESENT), (0, PRESENT), (0, ""), (0, EMPTY)]);
        Firewall::new(&TorConfig::default(), &runner)
            .remove()
            .unwrap();
        assert_eq!(
            runner.calls.borrow()[2].1,
            ["delete", "table", "inet", "omatorsurf"]
        );
    }
}
