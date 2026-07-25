use std::{io, path::PathBuf};

use thiserror::Error;

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, PskursorError>;

#[derive(Debug, Error)]
pub enum PskursorError {
    /// No usable gamepad was found while enumerating `/dev/input`.
    #[error("no controller/gamepad device connected or recognised")]
    NoControllerDeviceConnected,

    /// The daemon started but no device has been activated through the TUI yet.
    #[error(
        "no active device: `{path}` does not exist — run `pskursor --config`, \
         highlight your controller and press `y` to activate it"
    )]
    NoActiveDevice { path: PathBuf },

    /// Neither `$PSKURSOR_CONFIG`, `$XDG_CONFIG_HOME` nor `$HOME` were usable.
    #[error(
        "could not determine a config directory: set $PSKURSOR_CONFIG to an \
         explicit file path, or $XDG_CONFIG_HOME/$HOME to a directory"
    )]
    NoConfigDirectory,

    #[error("could not read config `{path}`")]
    ConfigRead {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("could not write config `{path}`")]
    ConfigWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("could not delete config `{path}`")]
    ConfigDelete {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("config `{path}` is not valid pskursor JSON")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to open evdev device `{path}`")]
    DeviceOpen {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Split out from [`Self::DeviceOpen`] because the fix is non-obvious:
    /// reading `/dev/input/event*` requires membership of the `input` group.
    #[error(
        "permission denied opening `{path}` — add yourself to the `input` group \
         (`sudo usermod -aG input $USER`, then re-login) or run pskursor as root"
    )]
    DevicePermissionDenied { path: PathBuf },

    /// The activated node exists but is not a gamepad, so it cannot drive the
    /// cursor. On a DualSense this usually means the *Touchpad* or *Motion
    /// Sensors* sibling node was activated instead of the gamepad node.
    #[error(
        "device `{name}` at `{path}` is not a usable gamepad ({missing}) — activate \
         the plain gamepad node instead of its Touchpad/Motion Sensors siblings"
    )]
    NotAGamepad {
        name: String,
        path: PathBuf,
        /// Which capability check failed, for a diagnosable message.
        missing: String,
    },

    #[error("lost connection to device `{name}` while reading events")]
    DeviceRead {
        name: String,
        #[source]
        source: io::Error,
    },

    /// Terminal setup / teardown and stdin handling for the TUI.
    #[error("terminal I/O error")]
    Terminal(#[source] io::Error),
}

impl PskursorError {
    /// Build the most informative error for a failed `Device::open`.
    pub(crate) fn from_open_failure(path: impl Into<PathBuf>, source: io::Error) -> Self {
        let path = path.into();
        match source.kind() {
            io::ErrorKind::PermissionDenied => Self::DevicePermissionDenied { path },
            _ => Self::DeviceOpen { path, source },
        }
    }
}

impl From<io::Error> for PskursorError {
    fn from(source: io::Error) -> Self {
        Self::Terminal(source)
    }
}
