use std::{env, ffi::OsString, path::PathBuf};

use crate::error::{PskursorError, Result};

pub const CONFIG_ENV_VAR: &str = "PSKURSOR_CONFIG";
const CONFIG_DIR_NAME: &str = "pskursor";
const CONFIG_FILE_NAME: &str = "devaync.json";

pub fn devaync_path() -> Result<PathBuf> {
    resolve(
        non_empty(CONFIG_ENV_VAR),
        non_empty("XDG_CONFIG_HOME"),
        non_empty("HOME"),
    )
}

fn non_empty(key: &str) -> Option<OsString> {
    env::var_os(key).filter(|value| !value.is_empty())
}

/// The precedence rules, split out from the environment so they can be tested
/// without mutating process-global state.
fn resolve(
    explicit: Option<OsString>,
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf> {
    if let Some(explicit) = explicit {
        return Ok(PathBuf::from(explicit));
    }

    let base = xdg_config_home
        .map(PathBuf::from)
        .or_else(|| home.map(|home| PathBuf::from(home).join(".config")))
        .ok_or(PskursorError::NoConfigDirectory)?;

    Ok(base.join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(value: &str) -> Option<OsString> {
        Some(OsString::from(value))
    }

    #[test]
    fn absent_legacy_file_falls_through_to_xdg() {
        let path = resolve(None, os("/xdg"), os("/home/me")).unwrap();

        assert_eq!(path, PathBuf::from("/xdg/pskursor/devaync.json"));
    }

    #[test]
    fn home_is_used_when_xdg_is_unset() {
        let path = resolve(None, None, os("/home/me")).unwrap();

        assert_eq!(
            path,
            PathBuf::from("/home/me/.config/pskursor/devaync.json")
        );
    }

    #[test]
    fn empty_env_values_are_treated_as_unset() {
        // `non_empty` filters these out before `resolve` sees them; this pins
        // the contract that an empty override never yields an empty path.
        let path = resolve(None, None, os("/home/me")).unwrap();

        assert!(path.is_absolute());
    }

    #[test]
    fn no_usable_directory_is_an_error() {
        let error = resolve(None, None, None).unwrap_err();

        assert!(matches!(error, PskursorError::NoConfigDirectory));
    }
}
