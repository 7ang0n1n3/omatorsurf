//! Read-only socket observations; UDP presence is not a successful DNS lookup.
use std::collections::HashSet;

use crate::{
    command::Runner,
    error::{AnonError, Result},
};

/// Fresh HTTPS using system routing, with curl config and proxies disabled.
pub fn tor_route(runner: &impl Runner) -> Result<String> {
    let output = runner
        .run(
            "curl",
            &[
                "--disable",
                "--noproxy",
                "*",
                "--proxy",
                "",
                "--ipv4",
                "--fail",
                "--silent",
                "--show-error",
                "--connect-timeout",
                "20",
                "--max-time",
                "40",
                "--proto",
                "=https",
                "https://check.torproject.org/api/ip",
            ],
        )?
        .checked("curl")?;
    parse_route(&output)
}

/// Bootstrap completion does not guarantee an immediately usable exit circuit.
/// Retry only transient transport failures, never an invalid/non-Tor response.
pub fn wait_for_tor_route(runner: &impl Runner) -> Result<String> {
    retry_route(runner, || {
        std::thread::sleep(std::time::Duration::from_secs(1))
    })
}
fn retry_route(runner: &impl Runner, mut pause: impl FnMut()) -> Result<String> {
    for attempt in 0..3 {
        match tor_route(runner) {
            Err(
                error @ AnonError::CommandFailed {
                    code: Some(6 | 7 | 28),
                    ..
                },
            ) if attempt < 2 => {
                tracing::warn!(%error, attempt = attempt + 1, "protected route is not yet reachable; retaining guard and retrying");
                pause();
            }
            result => return result,
        }
    }
    unreachable!("last attempt always returns")
}
fn parse_route(output: &str) -> Result<String> {
    let value: serde_json::Value =
        serde_json::from_str(output).map_err(|e| AnonError::Route(e.to_string()))?;
    if value["IsTor"] != true {
        return Err(AnonError::Route(
            "Tor Project did not confirm a Tor exit".into(),
        ));
    }
    let ip = value["IP"]
        .as_str()
        .and_then(|v| v.parse::<std::net::Ipv4Addr>().ok())
        .ok_or_else(|| AnonError::Route("invalid exit IPv4 address".into()))?;
    Ok(ip.to_string())
}

/// Observe sockets without connecting. TransPort expects redirected traffic;
/// direct TCP probes can exercise its invalid-destination handling.
pub fn tcp_listeners(runner: &impl Runner) -> Result<HashSet<u16>> {
    let output = runner
        .run("ss", &["-H", "-l", "-t", "-n", "-4"])?
        .checked("ss")?;
    Ok(parse_tcp_listeners(&output))
}

fn parse_tcp_listeners(output: &str) -> HashSet<u16> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            if fields.next()? != "LISTEN" {
                return None;
            }
            fields.nth(2)?.strip_prefix("127.0.0.1:")?.parse().ok()
        })
        .collect()
}

pub fn check_udp_port(runner: &impl Runner, port: u16) -> Result<bool> {
    let output = runner
        .run("ss", &["-H", "-l", "-u", "-n", "-4"])?
        .checked("ss")?;
    Ok(udp_listener_present(&output, port))
}

fn udp_listener_present(output: &str, port: u16) -> bool {
    let endpoint = format!("127.0.0.1:{port}");
    output
        .lines()
        .any(|line| line.split_whitespace().nth(3) == Some(endpoint.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RetryFake {
        calls: std::cell::Cell<u32>,
        non_tor: bool,
        permanent_failure: bool,
    }
    impl Runner for RetryFake {
        fn run(&self, _: &str, _: &[&str]) -> Result<crate::command::CommandOutput> {
            let call = self.calls.get() + 1;
            self.calls.set(call);
            Ok(crate::command::CommandOutput {
                code: Some(if !self.non_tor && (call < 3 || self.permanent_failure) {
                    28
                } else {
                    0
                }),
                stdout: format!(r#"{{"IsTor":{},"IP":"192.0.2.1"}}"#, !self.non_tor),
                stderr: "transport timeout".into(),
            })
        }
    }
    #[test]
    fn startup_transport_retries_are_bounded_and_non_tor_never_retried() {
        let runner = RetryFake {
            calls: std::cell::Cell::new(0),
            non_tor: false,
            permanent_failure: false,
        };
        assert!(retry_route(&runner, || {}).is_ok());
        assert_eq!(runner.calls.get(), 3);
        let runner = RetryFake {
            permanent_failure: true,
            ..runner
        };
        runner.calls.set(0);
        assert!(retry_route(&runner, || {}).is_err());
        assert_eq!(runner.calls.get(), 3);
        let runner = RetryFake {
            non_tor: true,
            permanent_failure: false,
            ..runner
        };
        runner.calls.set(0);
        assert!(retry_route(&runner, || {}).is_err());
        assert_eq!(runner.calls.get(), 1);
    }
    #[test]
    fn route_requires_tor_confirmation_and_valid_ip() {
        assert_eq!(
            parse_route(r#"{"IsTor":true,"IP":"192.0.2.1"}"#).unwrap(),
            "192.0.2.1"
        );
        for bad in [
            "{}",
            "bad",
            r#"{"IsTor":false,"IP":"192.0.2.1"}"#,
            r#"{"IsTor":true,"IP":"invalid"}"#,
        ] {
            assert!(parse_route(bad).is_err());
        }
    }

    #[test]
    fn udp_probe_requires_exact_loopback_endpoint() {
        assert!(udp_listener_present(
            "UNCONN 0 0 127.0.0.1:5353 0.0.0.0:*",
            5353
        ));
        for text in [
            "UNCONN 0 0 0.0.0.0:5353 0.0.0.0:*",
            "UNCONN 0 0 127.0.0.1:15353 0.0.0.0:*",
            "UNCONN 0 0 127.0.0.1:53 127.0.0.1:5353",
            "",
        ] {
            assert!(!udp_listener_present(text, 5353));
        }
    }

    #[test]
    fn tcp_observation_requires_listening_loopback_sockets() {
        let text = "LISTEN 0 4096 127.0.0.1:9040 0.0.0.0:*\nLISTEN 0 4096 127.0.0.1:9050 0.0.0.0:*\nLISTEN 0 4096 0.0.0.0:9051 0.0.0.0:*\nESTAB 0 0 127.0.0.1:9051 127.0.0.1:9000\nLISTEN 0 4096 127.0.0.1:19040 0.0.0.0:*";
        assert_eq!(
            parse_tcp_listeners(text),
            HashSet::from([9040, 9050, 19040])
        );
        assert!(parse_tcp_listeners("").is_empty());
    }
}
