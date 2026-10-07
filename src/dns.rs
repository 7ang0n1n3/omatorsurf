//! DNS protection uses nftables; resolver configuration is untouched.
use crate::{
    command::Runner,
    config::TorConfig,
    error::{AnonError, Result},
    firewall::Firewall,
    network,
};
pub fn enable(config: &TorConfig, runner: &impl Runner) -> Result<()> {
    if !Firewall::new(config, runner).verified()?
        || !network::check_udp_port(runner, config.dns_port)?
    {
        return Err(AnonError::Route(
            "DNS redirect or Tor DNS listener is missing".into(),
        ));
    }
    Ok(())
}
pub fn restore() { /* No resolver files were modified. */
}
