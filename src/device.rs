use regex::Regex;
use std::{collections::HashSet, error::Error, sync::LazyLock};

static MAC_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([0-9a-f]{2}:){5}[0-9a-f]{2}").unwrap());

#[derive(Debug)]
pub struct DeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_name: Option<String>,
    pub mac_addr: Option<String>,
    pub evdev_path: String,
    pub evdev_device: evdev::Device,
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
        if MAC_REGEX.is_match(physical_path) {
            device_mac_address = Some(physical_path.to_string());
        }

        devices.push(DeviceInfo {
            product_id: input_id.product(),
            vendor_id: input_id.vendor(),
            device_name: Some(name.to_owned()),
            mac_addr: device_mac_address,
            evdev_path: evdev_path.to_string_lossy().to_string(),
            evdev_device,
        });
    }

    Ok(devices)
}
