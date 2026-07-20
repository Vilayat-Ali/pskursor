use crate::device::DeviceInfo;
use std::thread;
use tracing::{error, info, trace};
use tracing_subscriber::EnvFilter;

pub fn start_daemon() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("prekursor daemon started...");

    if let Ok(mut evdev_device) = DeviceInfo::load_from_disk() {
        let name = evdev_device.name().unwrap_or("UNNAMED").to_string();
        info!(
            "Device found via evdev linux interface. Device name = {}",
            name
        );

        let worker_thread_handle = thread::spawn(move || {
            loop {
                match evdev_device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            trace!(
                                "Event: {:?} Code: {} Value: {}",
                                event.event_type(),
                                event.code(),
                                event.value()
                            );
                        }
                    }
                    Err(err) => {
                        error!("Device read failed (possibly disconnected): {}", err);
                        break;
                    }
                }
            }
        });

        if let Err(e) = worker_thread_handle.join() {
            error!("Worker thread panicked: {:?}", e);
        }
    } else {
        error!("Failed to load device from disk.");
    }
}
