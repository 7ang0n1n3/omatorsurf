use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, AnonError>;

#[derive(Debug, thiserror::Error)]
pub enum AnonError {
    #[error("Failed to read configuration '{}': {source}", path.display())]
    ConfigRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to parse configuration '{}': {source}", path.display())]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Runtime state operation failed: {0}")]
    Runtime(String),
    #[error("Anonymous route verification failed: {0}; protection rules remain installed")]
    Route(String),
    #[error("Failed to serialize status: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("Protection health checks failed; see the health report")]
    HealthFailed,
    #[error(
        "{program} is unavailable. Install {package} with: pkexec /usr/bin/pacman -S {package}"
    )]
    DependencyMissing {
        program: String,
        package: &'static str,
    },
    #[error("Failed to execute {program}: {source}")]
    CommandIo {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{program} exited with {code:?}: {stderr}")]
    CommandFailed {
        program: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("Tor service observation failed: {0}")]
    TorService(String),
    #[error(
        "Tor did not become ready within {0} seconds; inspect journalctl -u omatorsurf-tor.service"
    )]
    TorNotReady(u64),
    #[error("This operation requires root. Run: pkexec /usr/local/bin/omatorsurf {0}")]
    RootRequired(&'static str),
    #[error("Firewall operation failed: {0}")]
    Firewall(String),
    #[error(
        "Tor configuration does not match this application's configuration. Generate it with omatorsurf tor config and install it as /etc/omatorsurf/torrc"
    )]
    TorConfigMismatch,
    #[error(
        "Application Tor configuration is missing at /etc/omatorsurf/torrc. Launch scripts/install.sh through pkexec using its absolute path; the installer builds a missing release as your regular user"
    )]
    TorConfigMissing,
    #[error(
        "Tor startup failed: {startup}; stopping the newly started service also failed: {cleanup}"
    )]
    TorRollback {
        startup: Box<AnonError>,
        cleanup: Box<AnonError>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_error_preserves_path_and_source() {
        let error = AnonError::ConfigRead {
            path: PathBuf::from("/etc/omatorsurf/config.toml"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied"),
        };
        assert!(error.to_string().contains("/etc/omatorsurf/config.toml"));
        assert!(std::error::Error::source(&error).is_some());
    }
}
