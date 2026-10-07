use crate::error::{AnonError, Result};

pub fn require_root(operation: &'static str) -> Result<()> {
    let status =
        std::fs::read_to_string("/proc/self/status").map_err(|source| AnonError::CommandIo {
            program: "read effective UID".into(),
            source,
        })?;
    if effective_uid(&status) == Some(0) {
        Ok(())
    } else {
        Err(AnonError::RootRequired(operation))
    }
}

fn effective_uid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uses_effective_uid() {
        assert_eq!(effective_uid("Uid:\t1000\t0\t1000\t1000\n"), Some(0));
        assert_eq!(effective_uid("Uid:\t0\t1000\t0\t0\n"), Some(1000));
        assert_eq!(effective_uid("Uid: invalid"), None);
    }
}
