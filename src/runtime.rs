//! Informational state only. Live observations determine protection.
use crate::error::{AnonError, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};
pub const DIRECTORY: &str = "/run/omatorsurf";
#[derive(Debug, Serialize, Deserialize)]
pub struct RuntimeState {
    pub phase: String,
    pub last_error: Option<String>,
}
fn failure(error: impl std::fmt::Display) -> AnonError {
    AnonError::Runtime(error.to_string())
}
pub struct Lock {
    _file: File,
}
impl Lock {
    pub fn acquire() -> Result<Self> {
        fs::create_dir_all(DIRECTORY).map_err(failure)?;
        if !fs::symlink_metadata(DIRECTORY).map_err(failure)?.is_dir() {
            return Err(failure("runtime directory is not a directory"));
        }
        fs::set_permissions(DIRECTORY, fs::Permissions::from_mode(0o755)).map_err(failure)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .custom_flags(0x20000)
            .open(format!("{DIRECTORY}/lock"))
            .map_err(failure)?;
        file.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => {
                failure("another operation is running; retry when it finishes")
            }
            fs::TryLockError::Error(error) => failure(error),
        })?;
        Ok(Self { _file: file })
    }
}
pub fn record(phase: &str, error: Option<String>) -> Result<()> {
    write_state(
        Path::new(DIRECTORY),
        &RuntimeState {
            phase: phase.into(),
            last_error: error,
        },
    )
}
fn write_state(directory: &Path, state: &RuntimeState) -> Result<()> {
    let temporary = directory.join(format!("state.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&temporary)
            .map_err(failure)?;
        file.write_all(&serde_json::to_vec(state)?)
            .map_err(failure)?;
        file.sync_all().map_err(failure)?;
        fs::rename(&temporary, directory.join("state.json")).map_err(failure)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_is_atomic_and_contains_no_protection_claims() {
        let directory =
            std::env::temp_dir().join(format!("omatorsurf-state-test-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        write_state(
            &directory,
            &RuntimeState {
                phase: "failed".into(),
                last_error: Some("offline".into()),
            },
        )
        .unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join("state.json")).unwrap()).unwrap();
        assert_eq!(value["phase"], "failed");
        assert!(value.get("enabled").is_none());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
}
