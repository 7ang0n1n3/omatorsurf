use std::path::Path;

use crate::{
    cli::{FirewallCommands, TorCommands},
    command::{Runner, SystemRunner},
    config::Config,
    dns,
    error::{AnonError, Result},
    firewall::{self, Firewall},
    health::{HealthCheck, HealthReport},
    network, privilege, runtime,
    status::AnonStatus,
    tor::{self, Tor},
};

/// Owns orchestration and keeps Tor-only operations distinct from routing.
pub struct Controller {
    config: Config,
}

impl Controller {
    pub fn new(path: Option<&Path>) -> Result<Self> {
        let config = Config::load(path)?;
        tracing::debug!(
            verify_connection = config.general.verify_connection,
            "configuration validated"
        );
        Ok(Self { config })
    }

    pub fn start(&self) -> Result<()> {
        privilege::require_root("start")?;
        let _lock = runtime::Lock::acquire()?;
        let runner = SystemRunner;
        let tor = Tor::new(&self.config.tor, &runner);
        tor.validate_dependencies()?;
        runner.run("nft", &["--version"])?.checked("nft")?;
        runner.run("curl", &["--version"])?.checked("curl")?;
        runtime::record("starting", None)?;
        println!("Starting anonymous routing; installing protection before Tor...");
        let result = startup(&self.config, &runner, || tor.start());
        match result {
            Ok(()) => {
                runtime::record("active", None)?;
                println!("Anonymous routing enabled.");
                Ok(())
            }
            Err(error) => {
                // Never remove a guard installed before a failed Tor/route check.
                if let Err(state_error) = runtime::record("failed", Some(error.to_string())) {
                    tracing::error!(%state_error, "failed to record startup failure");
                }
                eprintln!(
                    "Startup failed. Any installed protection remains; retry start or explicitly stop to restore networking."
                );
                Err(error)
            }
        }
    }
    pub fn stop(&self) -> Result<()> {
        privilege::require_root("stop")?;
        let _lock = runtime::Lock::acquire()?;
        let runner = SystemRunner;
        // Stop application Tor first: a stop failure retains blocking rules.
        Tor::new(&self.config.tor, &runner).stop()?;
        dns::restore();
        Firewall::new(&self.config.tor, &runner).remove()?;
        runtime::record("disabled", None)?;
        println!("Anonymous routing disabled; normal networking restored.");
        Ok(())
    }
    pub fn status(&self) -> Result<AnonStatus> {
        privilege::require_root("status --json")?;
        observe(&self.config, &SystemRunner)
    }
    pub fn new_circuit(&self) -> Result<()> {
        privilege::require_root("new-circuit")?;
        let _lock = runtime::Lock::acquire()?;
        let runner = SystemRunner;
        if !observe(&self.config, &runner)?.enabled {
            return Err(AnonError::Route(
                "a verified protected route is required before requesting a new circuit".into(),
            ));
        }
        Tor::new(&self.config.tor, &runner).new_circuit()?;
        println!(
            "Tor accepted the new-circuit request. Existing connections keep their circuits; the exit IP may remain the same."
        );
        Ok(())
    }
    pub fn check(&self) -> Result<HealthReport> {
        privilege::require_root("check")?;
        health_report(&self.config, &SystemRunner)
    }

    pub fn tor_command(&self, command: TorCommands) -> Result<()> {
        let _lock = if matches!(command, TorCommands::Start | TorCommands::Stop) {
            privilege::require_root("tor start")?;
            Some(runtime::Lock::acquire()?)
        } else {
            None
        };
        let runner = SystemRunner;
        let tor = Tor::new(&self.config.tor, &runner);
        match command {
            TorCommands::Start => tor.start(),
            TorCommands::Stop => tor.stop(),
            TorCommands::Status { json } => {
                let status = tor.status()?;
                if json {
                    println!("{}", serde_json::to_string(&status)?);
                } else {
                    print!("{status}");
                }
                Ok(())
            }
            TorCommands::Config => {
                print!("{}", tor::render_config(&self.config.tor));
                Ok(())
            }
        }
    }

    pub fn firewall_command(&self, command: FirewallCommands) -> Result<()> {
        let _lock = if matches!(
            command,
            FirewallCommands::Apply
                | FirewallCommands::Guard
                | FirewallCommands::Remove
                | FirewallCommands::Check
        ) {
            privilege::require_root("firewall apply")?;
            Some(runtime::Lock::acquire()?)
        } else {
            None
        };
        let runner = SystemRunner;
        let firewall = Firewall::new(&self.config.tor, &runner);
        match command {
            FirewallCommands::Guard => {
                firewall.apply()?;
                if !firewall.verified()? {
                    return Err(AnonError::Firewall(
                        "guard inspection failed; installed rules remain".into(),
                    ));
                }
                println!("Fail-closed protection installed; Tor readiness is checked separately.");
            }
            FirewallCommands::Render => {
                print!(
                    "{}",
                    firewall::render(&self.config.tor, firewall::tor_uid(&runner)?)?
                );
            }
            FirewallCommands::Check => {
                privilege::require_root("firewall check")?;
                firewall.check()?;
                println!("Firewall transaction is valid; no rules were applied.");
            }
            FirewallCommands::Apply => {
                privilege::require_root("firewall apply")?;
                if !Tor::new(&self.config.tor, &runner).status()?.ready {
                    return Err(AnonError::Firewall(
                        "application Tor is not ready; run pkexec /usr/local/bin/omatorsurf tor start first".into(),
                    ));
                }
                firewall.apply()?;
                println!(
                    "Application firewall installed. Use check for live protection verification."
                );
            }
            FirewallCommands::Remove => {
                privilege::require_root("firewall remove")?;
                firewall.remove()?;
                println!(
                    "Application firewall removed. Direct networking is permitted again by this application."
                );
            }
            FirewallCommands::Status { json } => {
                let status = firewall.status()?;
                let verified = firewall.verified()?;
                if json {
                    let mut value = serde_json::to_value(&status)?;
                    value["rules_verified"] = verified.into();
                    println!("{}", value);
                } else {
                    println!(
                        "inet omatorsurf present: {}\nOwnership marker: {}\nComplete rules verified: {}",
                        status.table_present, status.managed, verified
                    );
                }
            }
        }
        Ok(())
    }
}

fn startup(
    config: &Config,
    runner: &impl Runner,
    start_tor: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let firewall = Firewall::new(&config.tor, runner);
    firewall.apply()?;
    if !firewall.verified()? {
        return Err(AnonError::Firewall(
            "installed protection rules do not match the expected rules".into(),
        ));
    }
    start_tor()?;
    dns::enable(&config.tor, runner)?;
    if config.general.verify_connection {
        network::wait_for_tor_route(runner)?;
    }
    let report = health_report(config, runner)?;
    if !report.passed() {
        eprint!("{report}");
        return Err(AnonError::HealthFailed);
    }
    Ok(())
}

fn observe(config: &Config, runner: &impl Runner) -> Result<AnonStatus> {
    let firewall = Firewall::new(&config.tor, runner);
    let presence = firewall.status()?;
    let verified = firewall.verified()?;
    let tor = Tor::new(&config.tor, runner).status()?;
    let mut errors = vec![];
    let ip = if verified && tor.ready {
        match network::tor_route(runner) {
            Ok(ip) => Some(ip),
            Err(e) => {
                errors.push(e.to_string());
                None
            }
        }
    } else {
        None
    };
    // Reobserve after network I/O; never trust the runtime state file.
    let verified = verified && firewall.verified()?;
    let current_tor = Tor::new(&config.tor, runner).status()?;
    let enabled = verified && current_tor.ready && ip.is_some();
    if presence.table_present && !verified {
        errors.push("application firewall exists but its complete rules are unverified".into());
    }
    if presence.table_present && !current_tor.ready {
        errors.push(
            "Tor is unavailable or not ready; retained rules continue blocking bypass traffic"
                .into(),
        );
    }
    Ok(AnonStatus {
        enabled,
        tor_running: current_tor.running,
        firewall_active: verified,
        killswitch: verified,
        dns_protected: verified,
        ipv6_protected: verified,
        public_ip: if enabled { ip } else { None },
        protection_state: if enabled {
            "protected"
        } else if presence.table_present {
            "degraded"
        } else {
            "disabled"
        },
        errors,
    })
}

fn health_report(config: &Config, runner: &impl Runner) -> Result<HealthReport> {
    let mut checks = vec![];
    for (name, program) in [
        ("Tor installed", "tor"),
        ("nftables installed", "nft"),
        ("curl installed", "curl"),
    ] {
        let result = runner
            .run(program, &["--version"])
            .and_then(|o| o.checked(program));
        checks.push(HealthCheck {
            name: name.into(),
            passed: result.is_ok(),
            required: true,
            detail: result.err().map(|e| e.to_string()).unwrap_or_default(),
        });
    }
    let tor = Tor::new(&config.tor, runner).status();
    for (name, passed) in [
        ("Tor service", tor.as_ref().is_ok_and(|s| s.running)),
        ("Tor bootstrap", tor.as_ref().is_ok_and(|s| s.bootstrapped)),
        (
            "SOCKS listener",
            tor.as_ref().is_ok_and(|s| s.socks_listening),
        ),
        (
            "Transparent listener",
            tor.as_ref().is_ok_and(|s| s.trans_listening),
        ),
        ("DNS listener", tor.as_ref().is_ok_and(|s| s.dns_listening)),
        (
            "Control listener",
            tor.as_ref().is_ok_and(|s| s.control_listening),
        ),
    ] {
        checks.push(HealthCheck {
            name: name.into(),
            passed,
            required: true,
            detail: tor
                .as_ref()
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default(),
        });
    }
    let firewall = Firewall::new(&config.tor, runner).verified();
    let verified = firewall.as_ref().is_ok_and(|v| *v);
    for name in [
        "Firewall rules",
        "Kill switch",
        "DNS redirect",
        "IPv6 protection",
    ] {
        checks.push(HealthCheck {
            name: name.into(),
            passed: verified,
            required: true,
            detail: firewall
                .as_ref()
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default(),
        });
    }
    let route = if verified && tor.as_ref().is_ok_and(|s| s.ready) {
        network::tor_route(runner)
    } else {
        Err(AnonError::Route("not probing an unverified route".into()))
    };
    for name in ["Internet / DNS lookup", "Tor route"] {
        checks.push(HealthCheck {
            name: name.into(),
            passed: route.is_ok(),
            required: true,
            detail: route
                .as_ref()
                .map(|ip| format!("Tor exit: {ip}"))
                .unwrap_or_else(|e| e.to_string()),
        });
    }
    let stable = Firewall::new(&config.tor, runner)
        .verified()
        .is_ok_and(|v| v)
        && Tor::new(&config.tor, runner)
            .status()
            .is_ok_and(|s| s.ready);
    checks.push(HealthCheck {
        name: "Protection recheck".into(),
        passed: stable,
        required: true,
        detail: "Reobserved after route verification".into(),
    });
    Ok(HealthReport { checks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandOutput;
    use std::cell::{Cell, RefCell};

    struct Fake {
        calls: RefCell<Vec<String>>,
        tor_ready: bool,
        route_ok: bool,
        corrupt_after_route: bool,
        probed: Cell<bool>,
        missing: Option<&'static str>,
        dns_available: bool,
    }
    impl Fake {
        fn ready() -> Self {
            Self {
                calls: RefCell::new(vec![]),
                tor_ready: true,
                route_ok: true,
                corrupt_after_route: false,
                probed: Cell::new(false),
                missing: None,
                dns_available: true,
            }
        }
    }
    impl Runner for Fake {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput> {
            if self.missing == Some(program) {
                return Err(AnonError::DependencyMissing {
                    program: program.into(),
                    package: if program == "tor" { "tor" } else { "nftables" },
                });
            }
            self.calls
                .borrow_mut()
                .push(format!("{program} {}", args.join(" ")));
            let output = match program {
                "nft" if args.contains(&"--stateless") => {
                    let text = include_str!("../nftables/expected.nft").replace("@TOR_UID@", "43").replace("@DNS_PORT@", "5353").replace("@TRANS_PORT@", "9040");
                    if self.corrupt_after_route && self.probed.get() { text.replace("policy drop", "policy accept") } else { text }
                }
                "nft" if args.contains(&"--json") => r#"{"nftables":[{"table":{"family":"inet","name":"omatorsurf","comment":"omatorsurf managed table v1"}}]}"#.into(),
                "getent" => "tor:x:43:43::/var/lib/tor:/usr/bin/nologin".into(),
                "systemctl" if self.tor_ready => "LoadState=loaded\nActiveState=active\nInvocationID=0123456789abcdef0123456789abcdef\n".into(),
                "systemctl" => "LoadState=loaded\nActiveState=inactive\nInvocationID=\n".into(),
                "ss" if args.contains(&"-t") => "LISTEN 0 0 127.0.0.1:9040 0.0.0.0:*\nLISTEN 0 0 127.0.0.1:9050 0.0.0.0:*\nLISTEN 0 0 127.0.0.1:9051 0.0.0.0:*".into(),
                "ss" if self.dns_available => "UNCONN 0 0 127.0.0.1:5353 0.0.0.0:*".into(),
                "ss" => String::new(),
                "journalctl" => "Bootstrapped 100% (done): Done".into(),
                "curl" if !args.contains(&"--version") => { self.probed.set(true); format!(r#"{{"IsTor":{},"IP":"192.0.2.1"}}"#, self.route_ok) },
                _ => String::new(),
            };
            Ok(CommandOutput {
                code: Some(0),
                stdout: output,
                stderr: String::new(),
            })
        }
        fn run_input(&self, program: &str, args: &[&str], _: &str) -> Result<CommandOutput> {
            self.run(program, args)
        }
    }

    #[test]
    fn startup_installs_guard_before_tor_and_retains_it_on_failure() {
        let runner = Fake::ready();
        assert!(
            startup(&Config::default(), &runner, || {
                assert!(runner.calls.borrow().contains(&"nft --file -".into()));
                Err(AnonError::TorNotReady(1))
            })
            .is_err()
        );
        assert!(
            !runner
                .calls
                .borrow()
                .iter()
                .any(|c| c.contains("delete table"))
        );
        assert!(!runner.probed.get());
    }

    #[test]
    fn protection_is_live_and_does_not_depend_on_runtime_cache() {
        let runner = Fake::ready();
        let status = observe(&Config::default(), &runner).unwrap();
        assert!(
            status.enabled && status.killswitch && status.dns_protected && status.ipv6_protected
        );
        assert_eq!(status.protection_state, "protected");
    }

    #[test]
    fn tor_death_retains_guard_and_does_not_probe_direct_internet() {
        let mut runner = Fake::ready();
        runner.tor_ready = false;
        let status = observe(&Config::default(), &runner).unwrap();
        assert!(!status.enabled && status.killswitch);
        assert_eq!(status.protection_state, "degraded");
        assert!(!runner.probed.get());
        assert!(!health_report(&Config::default(), &runner).unwrap().passed());
    }

    #[test]
    fn route_failure_or_rule_change_never_reports_protected() {
        for corruption in [false, true] {
            let mut runner = Fake::ready();
            runner.route_ok = corruption;
            runner.corrupt_after_route = corruption;
            let status = observe(&Config::default(), &runner).unwrap();
            assert!(!status.enabled);
            assert_eq!(status.public_ip, None);
            assert_eq!(status.protection_state, "degraded");
        }
    }

    #[test]
    fn missing_dependencies_fail_health_without_mutation() {
        for program in ["tor", "nft"] {
            let mut runner = Fake::ready();
            runner.missing = Some(program);
            assert!(!health_report(&Config::default(), &runner).unwrap().passed());
            assert!(
                !runner
                    .calls
                    .borrow()
                    .iter()
                    .any(|c| c.contains("--file") || c.contains("delete table"))
            );
        }
    }

    #[test]
    fn dns_failure_during_startup_retains_guard() {
        let mut runner = Fake::ready();
        runner.dns_available = false;
        assert!(startup(&Config::default(), &runner, || Ok(())).is_err());
        assert!(!runner.probed.get());
        assert!(
            !runner
                .calls
                .borrow()
                .iter()
                .any(|c| c.contains("delete table"))
        );
    }

    #[test]
    fn mutations_require_privilege_before_touching_system() {
        if privilege::require_root("start").is_ok() {
            return;
        }
        let controller = Controller {
            config: Config::default(),
        };
        for result in [
            controller.start(),
            controller.stop(),
            controller.new_circuit(),
        ] {
            assert!(matches!(result, Err(AnonError::RootRequired(_))));
        }
    }
}
