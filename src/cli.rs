use clap::{Parser, Subcommand};

use crate::{controller::Controller, error::Result};

#[derive(Debug, Parser)]
#[command(
    name = "omatorsurf",
    version,
    about = "Tor routing controller for Omarchy"
)]
pub struct Cli {
    /// System configuration file (defaults are used if the default file is absent).
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<std::path::PathBuf>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Enable anonymous routing with fail-closed startup (requires root).
    Start,
    /// Restore normal networking (requires root).
    Stop,
    /// Observe live protection status (requires root for nftables inspection).
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Request new circuits for future Tor connections (requires root).
    NewCircuit,
    /// Check live routing and protection (requires root).
    Check,
    /// Print the application version.
    Version,
    /// Control the application Tor service without enabling anonymous routing.
    Tor {
        #[command(subcommand)]
        command: TorCommands,
    },
    /// Inspect and manage the dedicated application firewall table.
    Firewall {
        #[command(subcommand)]
        command: FirewallCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum FirewallCommands {
    /// Render rules using configured ports and the installed Tor account UID.
    Render,
    /// Validate a prospective transaction without applying it (requires root).
    Check,
    /// Apply routing and blocking rules after checking Tor readiness (requires root).
    Apply,
    /// Install a fail-closed guard before Tor starts (requires root).
    Guard,
    /// Intentionally remove the application's firewall table (requires root).
    Remove,
    /// Inspect table ownership and complete rule correctness.
    Status {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum TorCommands {
    /// Start only application-managed Tor and wait for readiness (requires root).
    Start,
    /// Stop only application-managed Tor (requires root).
    Stop,
    /// Inspect the application Tor service and its listeners.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Print a Tor configuration matching the selected application configuration.
    Config,
}

impl Cli {
    pub fn dispatch(self) -> Result<()> {
        if matches!(self.command, Commands::Version) {
            println!("omatorsurf {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        let controller = Controller::new(self.config.as_deref())?;
        match self.command {
            Commands::Start => controller.start(),
            Commands::Stop => controller.stop(),
            Commands::Status { json } => {
                let status = controller.status()?;
                if json {
                    println!("{}", serde_json::to_string(&status)?);
                } else {
                    print!("{status}");
                }
                Ok(())
            }
            Commands::NewCircuit => controller.new_circuit(),
            Commands::Check => {
                let report = controller.check()?;
                print!("{report}");
                if report.passed() {
                    Ok(())
                } else {
                    Err(crate::error::AnonError::HealthFailed)
                }
            }
            Commands::Version => unreachable!("version handled before config loading"),
            Commands::Tor { command } => controller.tor_command(command),
            Commands::Firewall { command } => controller.firewall_command(command),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_commands_parse() {
        for command in ["start", "stop", "status", "new-circuit", "check", "version"] {
            assert!(Cli::try_parse_from(["omatorsurf", command]).is_ok());
        }
        assert!(matches!(
            Cli::try_parse_from(["omatorsurf", "status", "--json"])
                .unwrap()
                .command,
            Commands::Status { json: true }
        ));
    }

    #[test]
    fn rejects_invalid_commands_and_flags() {
        for args in [
            vec!["omatorsurf"],
            vec!["omatorsurf", "enable"],
            vec!["omatorsurf", "start", "--json"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }

    #[test]
    fn config_option_is_global() {
        let cli =
            Cli::try_parse_from(["omatorsurf", "status", "--config", "/tmp/custom.toml"]).unwrap();
        assert_eq!(
            cli.config.unwrap(),
            std::path::PathBuf::from("/tmp/custom.toml")
        );
    }

    #[test]
    fn tor_commands_parse_without_changing_routing_commands() {
        for command in ["start", "stop", "status", "config"] {
            assert!(Cli::try_parse_from(["omatorsurf", "tor", command]).is_ok());
        }
        assert!(matches!(
            Cli::try_parse_from(["omatorsurf", "tor", "status", "--json"])
                .unwrap()
                .command,
            Commands::Tor {
                command: TorCommands::Status { json: true }
            }
        ));
        assert!(Cli::try_parse_from(["omatorsurf", "tor", "start", "--json"]).is_err());
    }

    #[test]
    fn firewall_commands_parse() {
        for command in ["render", "check", "apply", "guard", "remove", "status"] {
            assert!(Cli::try_parse_from(["omatorsurf", "firewall", command]).is_ok());
        }
        assert!(matches!(
            Cli::try_parse_from(["omatorsurf", "firewall", "status", "--json"])
                .unwrap()
                .command,
            Commands::Firewall {
                command: FirewallCommands::Status { json: true }
            }
        ));
    }
}
