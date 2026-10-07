//! External commands use argument arrays and never invoke a shell or sudo.
use std::io::Write;
use std::process::{Command, Stdio};

use crate::error::{AnonError, Result};

pub struct CommandOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn checked(self, program: &str) -> Result<String> {
        if self.code == Some(0) {
            Ok(self.stdout)
        } else {
            let stdout = self.stdout.trim();
            let stderr = self.stderr.trim();
            let detail = match (stdout.is_empty(), stderr.is_empty()) {
                (true, true) => "command produced no diagnostic output".to_owned(),
                (true, false) => stderr.to_owned(),
                (false, true) => stdout.to_owned(),
                (false, false) => format!("{stderr}\nstdout: {stdout}"),
            };
            Err(AnonError::CommandFailed {
                program: program.into(),
                code: self.code,
                stderr: detail,
            })
        }
    }
}

pub trait Runner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput>;
    fn run_input(&self, program: &str, _args: &[&str], _input: &str) -> Result<CommandOutput> {
        Err(AnonError::Firewall(format!(
            "runner does not support stdin for {program}"
        )))
    }
}

pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput> {
        tracing::debug!(program, ?args, "executing command");
        let output = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|source| {
                if source.kind() == std::io::ErrorKind::NotFound {
                    AnonError::DependencyMissing {
                        program: program.into(),
                        package: match program {
                            "tor" => "tor",
                            "ss" => "iproute2",
                            "nft" => "nftables",
                            "getent" => "glibc",
                            "curl" => "curl",
                            "timeout" => "coreutils",
                            _ => "systemd",
                        },
                    }
                } else {
                    AnonError::CommandIo {
                        program: program.into(),
                        source,
                    }
                }
            })?;
        Ok(CommandOutput {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn run_input(&self, program: &str, args: &[&str], input: &str) -> Result<CommandOutput> {
        tracing::debug!(program, ?args, "executing command with stdin");
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| {
                if source.kind() == std::io::ErrorKind::NotFound {
                    AnonError::DependencyMissing {
                        program: program.into(),
                        package: "nftables",
                    }
                } else {
                    AnonError::CommandIo {
                        program: program.into(),
                        source,
                    }
                }
            })?;
        let write_result = child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(input.as_bytes());
        let output = child
            .wait_with_output()
            .map_err(|source| AnonError::CommandIo {
                program: program.into(),
                source,
            })?;
        if output.status.success() {
            write_result.map_err(|source| AnonError::CommandIo {
                program: format!("{program} stdin"),
                source,
            })?;
        }
        Ok(CommandOutput {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_failure_includes_status_and_stderr() {
        let error = CommandOutput {
            code: Some(1),
            stdout: String::new(),
            stderr: " access denied\n".into(),
        }
        .checked("systemctl")
        .unwrap_err();
        assert!(error.to_string().contains("access denied"));
        assert!(error.to_string().contains("Some(1)"));
    }
    #[test]
    fn signal_termination_is_failure() {
        assert!(
            CommandOutput {
                code: None,
                stdout: String::new(),
                stderr: String::new()
            }
            .checked("tor")
            .is_err()
        );
    }

    #[test]
    fn stdout_diagnostics_are_preserved_when_stderr_is_empty() {
        let error = CommandOutput {
            code: Some(1),
            stdout: "Failed to validate config: private data directory ownership\n".into(),
            stderr: String::new(),
        }
        .checked("tor")
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("private data directory ownership")
        );
    }

    #[test]
    fn failure_preserves_both_output_streams() {
        let error = CommandOutput {
            code: Some(1),
            stdout: "Tor warning".into(),
            stderr: "system error".into(),
        }
        .checked("tor")
        .unwrap_err();
        assert!(error.to_string().contains("Tor warning"));
        assert!(error.to_string().contains("system error"));
    }

    #[test]
    fn empty_failure_is_explicit() {
        let error = CommandOutput {
            code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
        }
        .checked("tor")
        .unwrap_err();
        assert!(error.to_string().contains("no diagnostic output"));
    }
}
