//! Device discovery, activation and evdev handle management.

use std::{collections::HashSet, fs, path::PathBuf, sync::LazyLock};

use evdev::{AbsoluteAxisCode, BusType, EventType, KeyCode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::{
    config::devaync_path,
    error::{PskursorError, Result},
};

static MAC_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b([0-9a-f]{2}:){5}[0-9a-f]{2}\b").unwrap());

/// Device names that are never useful as cursor controllers.
///
/// Matches ACPI-style identifiers such as `ELAN0662:00 04F3:30BC`, which some
/// firmware exposes verbatim instead of a product string.
static IGNORED_NAME_PATTERNS: LazyLock<Vec<Regex>> =
    LazyLock::new(|| vec![Regex::new(r"^[A-Z0-9]+:\d{2}\s+[0-9A-F]{4}:[0-9A-F]{4}$").unwrap()]);

/// Axes a gamepad must expose for pskursor to drive the cursor with it.
const CURSOR_AXES: [AbsoluteAxisCode; 2] = [AbsoluteAxisCode::ABS_X, AbsoluteAxisCode::ABS_Y];

/// Buttons that identify a node as a gamepad rather than a pointer.
///
/// Axes alone are not enough: a DualSense *Touchpad* node also reports ABS_X and
/// ABS_Y, so a capability check based only on axes happily nominates the
/// touchpad. `BTN_SOUTH` is `BTN_GAMEPAD`, the kernel's canonical marker for a
/// gamepad's action pad — touchpads and motion-sensor nodes never expose it.
const GAMEPAD_BUTTONS: [KeyCode; 2] = [KeyCode::BTN_SOUTH, KeyCode::BTN_START];

/// A discoverable input device, as shown in the TUI and persisted on activation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_name: Option<String>,
    pub mac_addr: Option<String>,
    pub evdev_path: String,

    /// Whether this node exposes the absolute axes needed to move the cursor.
    ///
    /// Composite controllers expose several nodes under near-identical names —
    /// a DualSense yields a gamepad node plus `… Motion Sensors` and
    /// `… Touchpad` siblings. Only the gamepad node can drive the cursor, so
    /// the TUI surfaces this flag to keep the wrong node from being activated.
    #[serde(default)]
    pub supports_cursor_control: bool,
}

impl DeviceInfo {
    /// Display name, falling back to a placeholder for unnamed devices.
    pub fn get_device_name(&self) -> String {
        match &self.device_name {
            Some(name) if !name.is_empty() => name.clone(),
            _ => String::from(" UNNAMED "),
        }
    }

    /// Display MAC address, falling back to a placeholder when unknown.
    pub fn get_mac_address(&self) -> String {
        match &self.mac_addr {
            Some(addr) if !addr.is_empty() => addr.clone(),
            _ => String::from(" - "),
        }
    }

    /// Open the underlying evdev node without checking its capabilities.
    ///
    /// Used by the TUI to read metadata for display. The daemon uses
    /// [`Self::open_for_cursor_control`] instead.
    pub fn open(&self) -> Result<evdev::Device> {
        evdev::Device::open(&self.evdev_path)
            .map_err(|source| PskursorError::from_open_failure(&self.evdev_path, source))
    }

    /// Open the node and verify it can actually drive the cursor.
    ///
    /// Re-checks capabilities rather than trusting the persisted
    /// `supports_cursor_control` flag: the config is a plain file a user can
    /// hand-edit, and event node numbering shifts across reconnects.
    pub fn open_for_cursor_control(&self) -> Result<evdev::Device> {
        let device = self.open()?;

        if let Some(missing) = missing_gamepad_capabilities(&device) {
            return Err(PskursorError::NotAGamepad {
                name: self.get_device_name(),
                path: PathBuf::from(&self.evdev_path),
                missing,
            });
        }

        Ok(device)
    }

    /// Persist this device as the active one, creating parent directories.
    pub fn activate(&self) -> Result<PathBuf> {
        let path = devaync_path()?;

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| PskursorError::ConfigWrite {
                path: path.clone(),
                source,
            })?;
        }

        let serialized = serde_json::to_vec_pretty(self).map_err(|source| {
            // Serializing a plain struct of owned primitives can only fail on
            // an I/O-shaped error, so report it as a write failure.
            PskursorError::ConfigWrite {
                path: path.clone(),
                source: source.into(),
            }
        })?;

        fs::write(&path, &serialized).map_err(|source| PskursorError::ConfigWrite {
            path: path.clone(),
            source,
        })?;

        info!(device = %self.get_device_name(), path = %path.display(), "device activated");
        Ok(path)
    }

    /// Read the currently activated device from disk.
    pub fn load_active() -> Result<Self> {
        let path = devaync_path()?;

        if !path.is_file() {
            return Err(PskursorError::NoActiveDevice { path });
        }

        let contents = fs::read_to_string(&path).map_err(|source| PskursorError::ConfigRead {
            path: path.clone(),
            source,
        })?;

        let device_info: Self =
            serde_json::from_str(contents.trim()).map_err(|source| PskursorError::ConfigParse {
                path: path.clone(),
                source,
            })?;

        debug!(path = %path.display(), evdev_path = %device_info.evdev_path, "loaded active device");
        Ok(device_info)
    }

    /// Whether an activated device is currently recorded on disk.
    pub fn is_active_device_set() -> bool {
        devaync_path().map(|path| path.is_file()).unwrap_or(false)
    }

    /// Clear the activated device.
    pub fn deactivate() -> Result<()> {
        let path = devaync_path()?;

        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            // Already absent is the desired end state.
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(PskursorError::ConfigDelete { path, source }),
        }
    }
}

/// Whether this node is a gamepad usable as a cursor controller.
pub fn is_gamepad(device: &evdev::Device) -> bool {
    missing_gamepad_capabilities(device).is_none()
}

/// Describe why a node cannot drive the cursor, or `None` if it can.
///
/// Requires both the X/Y axes the cursor is driven from and a gamepad action
/// button, so sibling nodes of a composite controller (touchpad, motion
/// sensors) are correctly rejected.
pub fn missing_gamepad_capabilities(device: &evdev::Device) -> Option<String> {
    let mut missing = Vec::new();

    if !device.supported_events().contains(EventType::ABSOLUTE) {
        missing.push("no absolute axes (EV_ABS)");
    } else if !device
        .supported_absolute_axes()
        .is_some_and(|axes| CURSOR_AXES.iter().all(|axis| axes.contains(*axis)))
    {
        missing.push("missing ABS_X/ABS_Y");
    }

    if !device
        .supported_keys()
        .is_some_and(|keys| GAMEPAD_BUTTONS.iter().any(|button| keys.contains(*button)))
    {
        missing.push("no gamepad buttons (BTN_SOUTH/BTN_START)");
    }

    (!missing.is_empty()).then(|| missing.join(", "))
}

/// Whether a device name should be hidden from the device list.
pub fn should_ignore_device_by_name(device_name: &str) -> bool {
    IGNORED_NAME_PATTERNS
        .iter()
        .any(|pattern| pattern.is_match(device_name))
}

/// Enumerate connected USB and Bluetooth input devices.
///
/// Cursor-capable devices are returned first so the intended controller node is
/// the default selection. Every node of a composite device is kept: collapsing
/// them by name would hide the very gamepad node the daemon needs.
pub fn get_devices() -> Result<Vec<DeviceInfo>> {
    let mut seen_paths = HashSet::<String>::new();
    let mut devices = Vec::new();

    for (evdev_path, evdev_device) in evdev::enumerate() {
        let Some(name) = evdev_device.name() else {
            continue;
        };

        if name.is_empty() || should_ignore_device_by_name(name) {
            continue;
        }

        let input_id = evdev_device.input_id();
        if !matches!(
            input_id.bus_type(),
            BusType::BUS_USB | BusType::BUS_BLUETOOTH
        ) {
            continue;
        }

        let evdev_path = evdev_path.to_string_lossy().to_string();
        if !seen_paths.insert(evdev_path.clone()) {
            continue;
        }

        // Prefer the stable `Uniq` identifier (which is the MAC for Bluetooth
        // controllers) and fall back to scanning the physical path.
        let mac_addr = evdev_device
            .unique_name()
            .or_else(|| evdev_device.physical_path())
            .filter(|candidate| MAC_REGEX.is_match(candidate))
            .map(str::to_owned);

        let supports_cursor_control = is_gamepad(&evdev_device);

        devices.push(DeviceInfo {
            vendor_id: input_id.vendor(),
            product_id: input_id.product(),
            device_name: Some(name.to_owned()),
            mac_addr,
            evdev_path,
            supports_cursor_control,
        });
    }

    // Stable ordering: cursor-capable first, then alphabetical by name.
    devices.sort_by(|a, b| {
        b.supports_cursor_control
            .cmp(&a.supports_cursor_control)
            .then_with(|| a.get_device_name().cmp(&b.get_device_name()))
    });

    if devices.is_empty() {
        warn!("no USB or Bluetooth input devices found");
    }

    Ok(devices)
}
