//! Controls only the dedicated application service, never the user's tor.service.
use std::{
    fmt, fs, thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::{
    command::Runner,
    config::TorConfig,
    error::{AnonError, Result},
    network,
};

pub const SERVICE: &str = "omatorsurf-tor.service";
const TORRC: &str = "/etc/omatorsurf/torrc";

#[derive(Debug, Serialize)]
pub struct TorStatus {
    pub service: &'static str,
    pub running: bool,
    pub bootstrapped: bool,
    pub socks_listening: bool,
    pub trans_listening: bool,
    pub dns_listening: bool,
    pub control_listening: bool,
    pub ready: bool,
}

impl fmt::Display for TorStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Tor service: {}", self.service)?;
        for (label, value) in [
            ("Running", self.running),
            ("Bootstrapped", self.bootstrapped),
            ("SOCKS listener", self.socks_listening),
            ("Transparent listener", self.trans_listening),
            ("DNS UDP listener", self.dns_listening),
            ("Control listener", self.control_listening),
            ("Ready", self.ready),
        ] {
            writeln!(f, "{label:<24}{}", if value { "YES" } else { "NO" })?;
        }
        writeln!(
            f,
            "Use omatorsurf status for system-wide protection status."
        )
    }
}

struct ServiceState {
    running: bool,
    invocation: String,
}

fn parse_service(output: &str) -> Result<ServiceState> {
    let property = |key: &str| output.lines().find_map(|line| line.strip_prefix(key));
    match property("LoadState=") {
        Some("loaded") => (),
        Some(state) => {
            return Err(AnonError::TorService(format!(
                "{SERVICE} is {state}; install systemd/{SERVICE} and run systemctl daemon-reload"
            )));
        }
        None => {
            return Err(AnonError::TorService(
                "systemctl returned no LoadState".into(),
            ));
        }
    }
    let running = match property("ActiveState=") {
        Some("active") => true,
        Some("inactive" | "failed" | "activating" | "deactivating" | "reloading") => false,
        _ => {
            return Err(AnonError::TorService(
                "systemctl returned an unrecognized ActiveState".into(),
            ));
        }
    };
    let invocation = property("InvocationID=").unwrap_or_default().to_owned();
    if running
        && (invocation.len() != 32 || !invocation.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(AnonError::TorService(
            "active service has no valid InvocationID".into(),
        ));
    }
    Ok(ServiceState {
        running,
        invocation,
    })
}

pub struct Tor<'a, R> {
    config: &'a TorConfig,
    runner: &'a R,
}

impl<'a, R: Runner> Tor<'a, R> {
    pub fn new(config: &'a TorConfig, runner: &'a R) -> Self {
        Self { config, runner }
    }

    pub fn validate_dependencies(&self) -> Result<()> {
        for (program, args) in [
            ("tor", vec!["--version"]),
            ("systemctl", vec!["--version"]),
            ("journalctl", vec!["--version"]),
            ("ss", vec!["--version"]),
        ] {
            self.runner.run(program, &args)?.checked(program)?;
        }
        Ok(())
    }

    pub fn new_circuit(&self) -> Result<()> {
        let port = self.config.control_port.to_string();
        // Stem handles the Tor protocol and cookie authentication externally.
        self.runner
            .run(
                "timeout",
                &[
                    "20",
                    "/usr/bin/python3",
                    "-I",
                    "-c",
                    include_str!("../scripts/new-circuit.py"),
                    &port,
                ],
            )?
            .checked("python3 / Stem")?;
        Ok(())
    }

    fn service_state(&self) -> Result<ServiceState> {
        let output = self
            .runner
            .run(
                "systemctl",
                &[
                    "--no-ask-password",
                    "show",
                    SERVICE,
                    "--property=LoadState,ActiveState,InvocationID",
                ],
            )?
            .checked("systemctl")?;
        parse_service(&output)
    }

    pub fn is_running(&self) -> Result<bool> {
        Ok(self.service_state()?.running)
    }

    pub fn status(&self) -> Result<TorStatus> {
        let state = self.service_state()?;
        let mut status = TorStatus {
            service: SERVICE,
            running: state.running,
            bootstrapped: false,
            socks_listening: false,
            trans_listening: false,
            dns_listening: false,
            control_listening: false,
            ready: false,
        };
        if !state.running {
            return Ok(status);
        }
        let listeners = network::tcp_listeners(self.runner)?;
        status.socks_listening = listeners.contains(&self.config.socks_port);
        status.trans_listening = listeners.contains(&self.config.trans_port);
        status.control_listening = listeners.contains(&self.config.control_port);
        status.dns_listening = network::check_udp_port(self.runner, self.config.dns_port)?;
        let filter = format!("_SYSTEMD_INVOCATION_ID={}", state.invocation);
        let logs = self
            .runner
            .run(
                "journalctl",
                &[
                    "--no-pager",
                    "--quiet",
                    "--unit",
                    SERVICE,
                    &filter,
                    "--output=cat",
                    "--lines=500",
                ],
            )?
            .checked("journalctl")?;
        status.bootstrapped = bootstrap_complete(&logs);
        // Avoid reporting ready after an observed service restart during probes.
        let current = self.service_state()?;
        status.running = current.running && current.invocation == state.invocation;
        status.ready = status.running
            && status.bootstrapped
            && status.socks_listening
            && status.trans_listening
            && status.dns_listening
            && status.control_listening;
        Ok(status)
    }

    pub fn start(&self) -> Result<()> {
        crate::privilege::require_root("tor start")?;
        self.validate_dependencies()?;
        let config = fs::read_to_string(TORRC).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                AnonError::TorConfigMissing
            } else {
                AnonError::ConfigRead {
                    path: TORRC.into(),
                    source,
                }
            }
        })?;
        if config != render_config(self.config) {
            return Err(AnonError::TorConfigMismatch);
        }
        // The dedicated unit validates through ExecStartPre as User=tor.
        // A root-run preflight would validate the Tor-owned private directory
        // under the wrong identity and may reject a correctly installed setup.
        self.start_validated(|| self.wait_until_ready())
    }

    fn start_validated(&self, wait: impl FnOnce() -> Result<()>) -> Result<()> {
        let already_running = self.is_running()?;
        if already_running {
            wait()?;
            println!("Application Tor is already ready.");
            return Ok(());
        }
        println!("Starting application Tor...");
        // Start synchronously: systemd's service timeout bounds startup itself.
        let result = self
            .runner
            .run("systemctl", &["--no-ask-password", "start", SERVICE])
            .and_then(|output| output.checked("systemctl").map(|_| ()))
            .and_then(|()| wait());
        if let Err(startup) = result {
            if let Err(cleanup) = self.stop_service() {
                return Err(AnonError::TorRollback {
                    startup: Box::new(startup),
                    cleanup: Box::new(cleanup),
                });
            }
            return Err(startup);
        }
        println!("Application Tor is ready.");
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        crate::privilege::require_root("tor stop")?;
        self.stop_service()?;
        println!("Application Tor stopped; the system tor.service was not changed.");
        Ok(())
    }

    fn stop_service(&self) -> Result<()> {
        self.runner
            .run("systemctl", &["--no-ask-password", "stop", SERVICE])?
            .checked("systemctl")?;
        if self.is_running()? {
            return Err(AnonError::TorService(
                "service still active after stop".into(),
            ));
        }
        Ok(())
    }

    pub fn wait_until_ready(&self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(self.config.readiness_timeout_seconds);
        loop {
            if self.status()?.ready {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(AnonError::TorNotReady(
                    self.config.readiness_timeout_seconds,
                ));
            }
            tracing::debug!("waiting for Tor listeners and bootstrap completion");
            thread::sleep(Duration::from_millis(500));
        }
    }
}

fn bootstrap_complete(logs: &str) -> bool {
    logs.lines()
        .any(|line| line.contains("Bootstrapped 100% (done):"))
}

pub fn render_config(config: &TorConfig) -> String {
    format!(
        "# Generated by omatorsurf tor config; do not edit by hand.\n\
DataDirectory /var/lib/omatorsurf/tor\n\
SocksPort 127.0.0.1:{}\n\
TransPort 127.0.0.1:{}\n\
DNSPort 127.0.0.1:{}\n\
ControlPort 127.0.0.1:{}\n\
CookieAuthentication 1\n\
CookieAuthFile /var/lib/omatorsurf/tor/control_auth_cookie\n\
ClientOnly 1\n\
AutomapHostsOnResolve 1\n\
VirtualAddrNetworkIPv4 10.192.0.0/10\n\
Log notice stdout\n",
        config.socks_port, config.trans_port, config.dns_port, config.control_port
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandOutput;
    use std::cell::RefCell;

    #[test]
    fn config_ports_are_generated_and_control_is_authenticated() {
        let config = TorConfig {
            socks_port: 9150,
            trans_port: 9140,
            dns_port: 5453,
            control_port: 9151,
            ..TorConfig::default()
        };
        let text = render_config(&config);
        for value in [
            "SocksPort 127.0.0.1:9150",
            "TransPort 127.0.0.1:9140",
            "DNSPort 127.0.0.1:5453",
            "ControlPort 127.0.0.1:9151",
            "CookieAuthentication 1",
        ] {
            assert!(text.contains(value));
        }
        assert_eq!(
            render_config(&TorConfig::default()),
            include_str!("../config/torrc")
        );
    }

    #[test]
    fn service_state_requires_loaded_unit_and_valid_observations() {
        assert!(
            !parse_service("LoadState=loaded\nActiveState=inactive\nInvocationID=\n")
                .unwrap()
                .running
        );
        assert!(parse_service("LoadState=loaded\nActiveState=active\nInvocationID=0123456789abcdef0123456789abcdef\n").unwrap().running);
        for text in [
            "",
            "LoadState=not-found\nActiveState=inactive",
            "LoadState=loaded\nActiveState=active\nInvocationID=",
            "LoadState=loaded\nActiveState=unknown",
        ] {
            assert!(parse_service(text).is_err());
        }
    }

    #[test]
    fn only_completed_bootstrap_is_ready() {
        assert!(bootstrap_complete(
            "[notice] Bootstrapped 100% (done): Done"
        ));
        assert!(!bootstrap_complete(
            "Bootstrapped 95% (circuit_create): Establishing a Tor circuit"
        ));
    }

    struct FakeRunner {
        calls: RefCell<Vec<(String, Vec<String>)>>,
        code: i32,
    }
    impl Runner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput> {
            self.calls.borrow_mut().push((
                program.into(),
                args.iter().map(|arg| (*arg).into()).collect(),
            ));
            Ok(CommandOutput {
                code: Some(self.code),
                stdout: "LoadState=loaded\nActiveState=inactive\nInvocationID=\n".into(),
                stderr: "denied".into(),
            })
        }
    }

    #[test]
    fn inactive_service_never_probes_or_claims_readiness() {
        let runner = FakeRunner {
            calls: RefCell::new(vec![]),
            code: 0,
        };
        let config = TorConfig::default();
        let status = Tor::new(&config, &runner).status().unwrap();
        assert!(!status.ready);
        assert!(!status.running);
        assert_eq!(runner.calls.borrow().len(), 1);
        assert!(runner.calls.borrow()[0].1.contains(&SERVICE.to_owned()));
    }

    #[test]
    fn failed_observation_is_an_error_not_a_stopped_service() {
        let runner = FakeRunner {
            calls: RefCell::new(vec![]),
            code: 1,
        };
        assert!(
            Tor::new(&TorConfig::default(), &runner)
                .is_running()
                .is_err()
        );
    }

    #[test]
    fn cleanup_targets_only_application_service() {
        let runner = FakeRunner {
            calls: RefCell::new(vec![]),
            code: 0,
        };
        Tor::new(&TorConfig::default(), &runner)
            .stop_service()
            .unwrap();
        let calls = runner.calls.borrow();
        assert_eq!(
            calls[0],
            (
                "systemctl".into(),
                vec!["--no-ask-password".into(), "stop".into(), SERVICE.into()]
            )
        );
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn failed_readiness_stops_a_newly_started_service() {
        let runner = FakeRunner {
            calls: RefCell::new(vec![]),
            code: 0,
        };
        let config = TorConfig::default();
        assert!(matches!(
            Tor::new(&config, &runner).start_validated(|| Err(AnonError::TorNotReady(1))),
            Err(AnonError::TorNotReady(1))
        ));
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 4);
        assert!(calls[1].1.contains(&"start".to_owned()));
        assert!(calls[2].1.contains(&"stop".to_owned()));
    }

    struct ActiveRunner {
        calls: RefCell<Vec<Vec<String>>>,
    }
    impl Runner for ActiveRunner {
        fn run(&self, _: &str, args: &[&str]) -> Result<CommandOutput> {
            self.calls
                .borrow_mut()
                .push(args.iter().map(|arg| (*arg).into()).collect());
            Ok(CommandOutput { code: Some(0), stdout: "LoadState=loaded\nActiveState=active\nInvocationID=0123456789abcdef0123456789abcdef\n".into(), stderr: String::new() })
        }
    }

    #[test]
    fn repeated_start_never_restarts_or_stops_an_active_service() {
        let runner = ActiveRunner {
            calls: RefCell::new(vec![]),
        };
        let config = TorConfig::default();
        let tor = Tor::new(&config, &runner);
        tor.start_validated(|| Ok(())).unwrap();
        assert!(
            tor.start_validated(|| Err(AnonError::TorNotReady(1)))
                .is_err()
        );
        assert_eq!(runner.calls.borrow().len(), 2);
        assert!(
            runner
                .calls
                .borrow()
                .iter()
                .all(|args| args.contains(&"show".to_owned()))
        );
    }

    struct CleanupFailure;
    impl Runner for CleanupFailure {
        fn run(&self, _: &str, args: &[&str]) -> Result<CommandOutput> {
            Ok(CommandOutput {
                code: Some(if args.contains(&"stop") { 1 } else { 0 }),
                stdout: "LoadState=loaded\nActiveState=inactive\nInvocationID=\n".into(),
                stderr: "stop denied".into(),
            })
        }
    }

    #[test]
    fn rollback_failure_preserves_both_errors() {
        let config = TorConfig::default();
        let error = Tor::new(&config, &CleanupFailure)
            .start_validated(|| Err(AnonError::TorNotReady(1)))
            .unwrap_err();
        assert!(matches!(error, AnonError::TorRollback { .. }));
        assert!(error.to_string().contains("stop denied"));
        assert!(error.to_string().contains("did not become ready"));
    }

    struct ReadyRunner {
        calls: RefCell<Vec<(String, Vec<String>)>>,
        bootstrapped: bool,
    }
    impl Runner for ReadyRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput> {
            self.calls.borrow_mut().push((
                program.into(),
                args.iter().map(|arg| (*arg).into()).collect(),
            ));
            let stdout = match program {
                "systemctl" => {
                    "LoadState=loaded\nActiveState=active\nInvocationID=0123456789abcdef0123456789abcdef\n"
                }
                "ss" if args.contains(&"-t") => {
                    "LISTEN 0 4096 127.0.0.1:9050 0.0.0.0:*\nLISTEN 0 4096 127.0.0.1:9040 0.0.0.0:*\nLISTEN 0 4096 127.0.0.1:9051 0.0.0.0:*\n"
                }
                "ss" => "UNCONN 0 0 127.0.0.1:5353 0.0.0.0:*\n",
                "journalctl" if self.bootstrapped => "Bootstrapped 100% (done): Done\n",
                "journalctl" => "Bootstrapped 5% (conn): Connecting to a relay\n",
                _ => panic!("unexpected program {program}"),
            };
            Ok(CommandOutput {
                code: Some(0),
                stdout: stdout.into(),
                stderr: String::new(),
            })
        }
    }

    #[test]
    fn readiness_observes_transparent_listener_without_connecting() {
        let runner = ReadyRunner {
            calls: RefCell::new(vec![]),
            bootstrapped: true,
        };
        let config = TorConfig::default();
        assert!(Tor::new(&config, &runner).status().unwrap().ready);
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 5);
        assert_eq!(
            calls[1],
            (
                "ss".into(),
                vec![
                    "-H".into(),
                    "-l".into(),
                    "-t".into(),
                    "-n".into(),
                    "-4".into()
                ]
            )
        );
        assert_eq!(calls[2].0, "ss");
        assert_eq!(calls[3].0, "journalctl");
    }

    #[test]
    fn listeners_alone_do_not_prove_bootstrap() {
        let runner = ReadyRunner {
            calls: RefCell::new(vec![]),
            bootstrapped: false,
        };
        let config = TorConfig::default();
        let status = Tor::new(&config, &runner).status().unwrap();
        assert!(status.trans_listening);
        assert!(!status.ready);
    }
}
