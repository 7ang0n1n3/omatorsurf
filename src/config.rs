use std::{collections::HashSet, fs, path::Path};

use serde::Deserialize;

use crate::error::{AnonError, Result};

pub const DEFAULT_CONFIG_PATH: &str = "/etc/omatorsurf/config.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub tor: TorConfig,
    pub network: NetworkConfig,
    pub general: GeneralConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TorConfig {
    pub socks_port: u16,
    pub trans_port: u16,
    pub dns_port: u16,
    pub control_port: u16,
    pub readiness_timeout_seconds: u64,
}

impl Default for TorConfig {
    fn default() -> Self {
        Self {
            socks_port: 9050,
            trans_port: 9040,
            dns_port: 5353,
            control_port: 9051,
            readiness_timeout_seconds: 120,
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Ipv6Policy {
    Block,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkConfig {
    pub ipv6: Ipv6Policy,
    pub killswitch: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            ipv6: Ipv6Policy::Block,
            killswitch: true,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GeneralConfig {
    pub verify_connection: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            verify_connection: true,
        }
    }
}

impl Config {
    pub fn load(explicit_path: Option<&Path>) -> Result<Self> {
        let path = explicit_path.unwrap_or_else(|| Path::new(DEFAULT_CONFIG_PATH));
        tracing::debug!(path = %path.display(), "loading configuration");
        let config = match fs::read_to_string(path) {
            Ok(contents) => Self::parse(&contents, path)?,
            Err(error)
                if explicit_path.is_none() && error.kind() == std::io::ErrorKind::NotFound =>
            {
                tracing::debug!("system configuration absent; using defaults");
                Self::default()
            }
            Err(source) => {
                return Err(AnonError::ConfigRead {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        config.validate()?;
        Ok(config)
    }

    fn parse(contents: &str, path: &Path) -> Result<Self> {
        let config: Self = toml::from_str(contents).map_err(|source| AnonError::ConfigParse {
            path: path.to_owned(),
            source,
        })?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        let ports = [
            self.tor.socks_port,
            self.tor.trans_port,
            self.tor.dns_port,
            self.tor.control_port,
        ];
        if ports.contains(&0) {
            return Err(AnonError::InvalidConfig(
                "Tor ports must be between 1 and 65535".into(),
            ));
        }
        if ports.into_iter().collect::<HashSet<_>>().len() != ports.len() {
            return Err(AnonError::InvalidConfig(
                "Tor listener ports must be distinct".into(),
            ));
        }
        if !self.network.killswitch {
            return Err(AnonError::InvalidConfig(
                "the kill switch is mandatory; set network.killswitch = true".into(),
            ));
        }
        if !(1..=600).contains(&self.tor.readiness_timeout_seconds) {
            return Err(AnonError::InvalidConfig(
                "tor.readiness_timeout_seconds must be between 1 and 600".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Result<Config> {
        Config::parse(input, Path::new("test.toml"))
    }

    #[test]
    fn defaults_are_safe_and_sample_parses() {
        let defaults = parse("").unwrap();
        assert_eq!(defaults.tor.socks_port, 9050);
        assert_eq!(defaults.network.ipv6, Ipv6Policy::Block);
        assert!(defaults.network.killswitch);
        assert!(defaults.general.verify_connection);
        parse(include_str!("../config/omatorsurf.toml")).unwrap();
    }

    #[test]
    fn partial_configuration_uses_defaults() {
        let config = parse("[tor]\nsocks_port = 9150").unwrap();
        assert_eq!(config.tor.socks_port, 9150);
        assert_eq!(config.tor.trans_port, 9040);
    }

    #[test]
    fn unsafe_or_invalid_configuration_is_rejected() {
        for input in [
            "[tor]\nsocks_port = 0",
            "[tor]\nsocks_port = 9040",
            "[tor]\nsocks_port = 65536",
            "[network]\nkillswitch = false",
            "[network]\nipv6 = 'allow'",
            "[tor]\nsock_port = 9050",
            "unknown = true",
            "[general]\nverify_connection = 'yes'",
            "[broken",
            "[tor]\nreadiness_timeout_seconds = 0",
            "[tor]\nreadiness_timeout_seconds = 601",
        ] {
            assert!(parse(input).is_err(), "accepted {input}");
        }
    }

    #[test]
    fn explicit_missing_configuration_is_an_error() {
        assert!(matches!(
            Config::load(Some(Path::new("/dev/null/omatorsurf.toml"))),
            Err(AnonError::ConfigRead { .. })
        ));
    }
}
