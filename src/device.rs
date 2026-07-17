use hidapi::HidApi;
use regex::Regex;
use std::{
    collections::HashSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

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

fn find_event_device(device_name: &str) -> Option<PathBuf> {
    let sys_path = Path::new("/sys/class/input");
    let query = device_name.to_lowercase();

    if let Ok(entries) = fs::read_dir(sys_path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if file_name_str.starts_with("event") {
                let name_file_path = entry.path().join("device/name");
                if let Ok(name) = fs::read_to_string(name_file_path)
                    && name.trim().to_lowercase().contains(&query)
                {
                    return Some(Path::new("/dev/input").join(file_name_str.as_ref()));
                }
            }
        }
    }
    None
}

pub fn get_devices() -> Result<Vec<DeviceInfo>, Box<dyn Error>> {
    let api = HidApi::new()?;
    let mut device_name_set = HashSet::<String>::new();
    let mut devices = Vec::new();

    for device in api.device_list() {
        let name = device.product_string().unwrap_or(" UNNAMED ");

        if name.is_empty() || name == " UNNAMED " {
            continue;
        }

        if should_ignore_device_by_name(name) {
            continue;
        }

        if device_name_set.insert(name.to_string())
            && let Some(device_evdev_path) = find_event_device(name)
            && let Ok(evdev_device) = evdev::Device::open(&device_evdev_path)
        {
            devices.push(DeviceInfo {
                vendor_id: device.vendor_id(),
                product_id: device.product_id(),
                device_name: Some(name.to_owned()),
                mac_addr: device.serial_number().map(|s| s.to_owned()),
                evdev_path: device_evdev_path.to_string_lossy().to_string(),
                evdev_device,
            });
        }
    }

    Ok(devices)
}
