use std::fmt;

/// Live, read-only system observations; every required failure fails the report.
#[derive(Debug)]
pub struct HealthReport {
    pub checks: Vec<HealthCheck>,
}

#[derive(Debug)]
pub struct HealthCheck {
    pub name: String,
    pub passed: bool,
    pub required: bool,
    pub detail: String,
}

impl HealthReport {
    pub fn passed(&self) -> bool {
        self.checks.iter().any(|check| check.required)
            && self
                .checks
                .iter()
                .filter(|check| check.required)
                .all(|check| check.passed)
    }
}

impl fmt::Display for HealthReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Omarchy Anon Health Check\n")?;
        for check in &self.checks {
            writeln!(
                f,
                "{:<24}{} {}",
                check.name,
                if check.passed { "OK" } else { "FAIL" },
                check.detail
            )?;
        }
        writeln!(
            f,
            "\nChecks: {}",
            if self.passed() { "PASSED" } else { "FAILED" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_or_optional_only_reports_cannot_pass() {
        assert!(!HealthReport { checks: vec![] }.passed());
        assert!(
            !HealthReport {
                checks: vec![HealthCheck {
                    name: "optional".into(),
                    passed: true,
                    required: false,
                    detail: String::new()
                }]
            }
            .passed()
        );
    }

    #[test]
    fn required_failure_fails_report() {
        let mut report = HealthReport {
            checks: vec![HealthCheck {
                name: "Tor".into(),
                passed: false,
                required: true,
                detail: "unavailable".into(),
            }],
        };
        assert!(!report.passed());
        report.checks[0].passed = true;
        assert!(report.passed());
    }
}
