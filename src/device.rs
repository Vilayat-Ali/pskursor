use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    error::Error,
    fs,
    io::{self, Read, Write},
    sync::LazyLock,
};
use tracing::info;

/// Path to the DEVAYNC configuration file.
///
/// This file is used as the synchronization point between the terminal user
/// interface (TUI) and the background daemon.
///
/// When a device is selected in the TUI, its information is serialized into
/// this JSON file. The daemon monitors this file to determine which device
/// should become active.
///
/// It stores the json stringified version of [`DeviceInfo`]
///
/// **DEVAYNC** stands for **Device Activation Synchronization**.
pub const DEVAYNC_FILE_PATH: &str = "./devaync.json";

static MAC_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([0-9a-f]{2}:){5}[0-9a-f]{2}").unwrap());

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_name: Option<String>,
    pub mac_addr: Option<String>,
    pub evdev_path: String,
}

impl DeviceInfo {
    pub fn get_mac_address(&self) -> String {
        match &self.mac_addr {
            Some(addr) if !addr.is_empty() => addr.clone(),
            _ => String::from(" - "),
        }
    }

    pub fn get_device_name(&self) -> String {
        match &self.device_name {
            Some(name) if !name.is_empty() => name.clone(),
            _ => String::from(" UNNAMED "),
        }
    }

    pub fn get_evdev_device(&self) -> Result<evdev::Device, io::Error> {
        let device = evdev::Device::open(&self.evdev_path)?;
        Ok(device)
    }

    pub fn save_config_on_disk(&self) -> Result<(), std::io::Error> {
        let mut config_file = fs::File::create(DEVAYNC_FILE_PATH)?;
        let config_file_content = serde_json::to_vec_pretty(self)?;
        config_file.write_all(&config_file_content)?;
        Ok(())
    }

    pub fn load_from_disk() -> Result<evdev::Device, io::Error> {
        if !Self::exists() {
            info!("No devaync file found. Daemon is idle...");
        }

        let mut buffer = String::with_capacity(64);
        let mut devaync_file = fs::File::open(DEVAYNC_FILE_PATH)?;
        devaync_file.read_to_string(&mut buffer)?;

        let device_info = serde_json::from_str::<DeviceInfo>(buffer.trim())?;
        let evdev_device = evdev::Device::open(device_info.evdev_path)?;

        Ok(evdev_device)
    }

    pub fn exists() -> bool {
        if let Ok(devaync_file_exists) = fs::exists(DEVAYNC_FILE_PATH) {
            return devaync_file_exists;
        }

        false
    }

    pub fn remove_from_disk() -> Result<(), io::Error> {
        fs::remove_file(DEVAYNC_FILE_PATH)?;
        Ok(())
    }
}

fn get_device_names_to_ignore() -> Vec<Regex> {
    vec![
        // This regex looks for ACPI-style patterns.
        // Note: HIDAPI product strings usually yield clean names like "ELAN Touchpad"
        // rather than the raw ACPI IDs, but keeping it here corrected.
        Regex::new(r"^[A-Z0-9]+:\d{2}\s+[0-9A-F]{4}:[0-9A-F]{4}$").unwrap(),
    ]
}

pub fn should_ignore_device_by_name(device_name: &str) -> bool {
    for regex in get_device_names_to_ignore() {
        if regex.is_match(device_name) {
            return true;
        }
    }
    false
}

pub fn get_devices() -> Result<Vec<DeviceInfo>, Box<dyn Error>> {
    let mut device_name_set = HashSet::<String>::new();
    let mut devices = Vec::new();

    for (evdev_path, evdev_device) in evdev::enumerate() {
        let Some(name) = evdev_device.name() else {
            continue;
        };

        if name.is_empty() || should_ignore_device_by_name(name) {
            continue;
        }

        let input_id = evdev_device.input_id();
        let bus_type = input_id.bus_type();

        if bus_type != evdev::BusType::BUS_USB && bus_type != evdev::BusType::BUS_BLUETOOTH {
            continue;
        }

        if !device_name_set.insert(name.to_string()) {
            continue;
        }

        let mut device_mac_address: Option<String> = Option::None;
        let physical_path = evdev_device.physical_path().unwrap_or_default();
        if MAC_REGEX.find(physical_path).is_some() {
            device_mac_address = Some(physical_path.to_string());
        }

        devices.push(DeviceInfo {
            product_id: input_id.product(),
            vendor_id: input_id.vendor(),
            device_name: Some(name.to_owned()),
            mac_addr: device_mac_address,
            evdev_path: evdev_path.to_string_lossy().to_string(),
        });
    }

    Ok(devices)
}
