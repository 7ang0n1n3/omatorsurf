use std::fmt;

use serde::Serialize;

/// Stable frontend fields constructed from actual observations.
#[derive(Debug, Serialize)]
pub struct AnonStatus {
    pub enabled: bool,
    pub tor_running: bool,
    pub firewall_active: bool,
    pub killswitch: bool,
    pub dns_protected: bool,
    pub ipv6_protected: bool,
    pub public_ip: Option<String>,
    pub protection_state: &'static str,
    pub errors: Vec<String>,
}

impl fmt::Display for AnonStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Omarchy Anon\n")?;
        for (label, value) in [
            ("Status", self.protection_state),
            (
                "Tor",
                if self.tor_running {
                    "RUNNING"
                } else {
                    "STOPPED"
                },
            ),
            ("Firewall", active(self.firewall_active)),
            (
                "Kill switch",
                if self.killswitch {
                    "ENABLED"
                } else {
                    "DISABLED"
                },
            ),
            ("DNS protection", active(self.dns_protected)),
            ("IPv6 protection", active(self.ipv6_protected)),
            ("Public IP", self.public_ip.as_deref().unwrap_or("UNKNOWN")),
        ] {
            writeln!(f, "{label:<20}{value}")?;
        }
        for error in &self.errors {
            writeln!(f, "Detail: {error}")?;
        }
        Ok(())
    }
}

fn active(value: bool) -> &'static str {
    if value { "ACTIVE" } else { "INACTIVE" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AnonStatus {
        AnonStatus {
            enabled: false,
            tor_running: false,
            firewall_active: false,
            killswitch: false,
            dns_protected: false,
            ipv6_protected: false,
            public_ip: None,
            protection_state: "disabled",
            errors: vec![],
        }
    }

    #[test]
    fn json_preserves_exact_api_fields_and_types() {
        assert_eq!(
            serde_json::to_value(sample()).unwrap(),
            serde_json::json!({
                "enabled": false, "tor_running": false, "firewall_active": false,
                "killswitch": false, "dns_protected": false, "ipv6_protected": false,
                "public_ip": null, "protection_state": "disabled", "errors": []
            })
        );
        let mut status = sample();
        status.public_ip = Some("192.0.2.1".into());
        assert_eq!(
            serde_json::to_value(status).unwrap()["public_ip"],
            "192.0.2.1"
        );
    }

    #[test]
    fn schema_documents_every_field_and_json_type() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../config/status.schema.json")).unwrap();
        let value = serde_json::to_value(sample()).unwrap();
        for field in schema["required"].as_array().unwrap() {
            assert!(value.get(field.as_str().unwrap()).is_some());
        }
        for (field, value) in value.as_object().unwrap() {
            let expected = &schema["properties"][field]["type"];
            let actual = match value {
                serde_json::Value::Bool(_) => "boolean",
                serde_json::Value::String(_) => "string",
                serde_json::Value::Null => "null",
                serde_json::Value::Array(_) => "array",
                _ => panic!("unexpected type"),
            };
            assert!(
                expected == actual
                    || expected
                        .as_array()
                        .is_some_and(|types| types.iter().any(|v| v == actual))
            );
        }
    }

    #[test]
    fn text_renders_unknown_ip_and_unprotected_state() {
        let text = sample().to_string();
        assert!(text.contains("disabled"));
        assert!(text.contains("UNKNOWN"));
    }
}
