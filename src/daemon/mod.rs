pub mod mouse_controller;

use evdev::{AbsInfo, AbsoluteAxisCode, EventSummary, KeyCode};
use std::{collections::HashMap, sync::mpsc, thread};
use tracing::{debug, info, trace, warn};

use crate::{
    daemon::mouse_controller::translate_controller_input_to_mouse_movements,
    device::DeviceInfo,
    error::{PskursorError, Result},
};

#[derive(Debug)]
pub enum ButtonAction {
    RELEASED,
    PRESSED,
    REPEATED,
}

pub enum InputEvent {
    Axis {
        axis: AbsoluteAxisCode,
        value: f32,
        centred: bool,
    },
    Button {
        key: KeyCode,
        action: ButtonAction,
    },
}

/// Fraction of an axis' half-range treated as centre when the kernel reports no
/// `flat` value of its own.
const FALLBACK_DEADZONE_RATIO: f32 = 0.12;

/// Start the daemon: load the activated device and pump its event stream.
pub fn start_daemon() -> Result<()> {
    info!("pskursor daemon started");

    let device_info = DeviceInfo::load_active()?;
    let mut device = device_info.open_for_cursor_control()?;

    let name = device
        .name()
        .map(str::to_owned)
        .unwrap_or_else(|| device_info.get_device_name());

    info!(
        device = %name,
        evdev_path = %device_info.evdev_path,
        physical_path = device.physical_path().unwrap_or("-"),
        "controller acquired via evdev"
    );

    let axes = AxisTable::probe(&device);
    axes.log_summary();

    info!("streaming controller events — move a stick or press a button");

    let (tx, rx) = mpsc::channel::<InputEvent>();

    let _input_listening_thread_handle = thread::spawn(move || -> Result<()> {
        info!(
            "Worker thread spawned for listening device input stream at {}",
            device_info.evdev_path
        );

        loop {
            let events = device
                .fetch_events()
                .map_err(|source| PskursorError::DeviceRead {
                    name: name.clone(),
                    source,
                })?;

            for event in events {
                trace!(
                    event_type = ?event.event_type(),
                    code = event.code(),
                    value = event.value(),
                    "raw evdev event"
                );

                match event.destructure() {
                    EventSummary::AbsoluteAxis(_, axis, value) => {
                        let (normalized, centred) = axes.normalize(axis, value);

                        if centred {
                            trace!(axis = ?axis, value, "axis at rest");
                        } else {
                            debug!(
                                axis = ?axis,
                                value,
                                normalized = format_args!("{normalized:+.3}"),
                                "joystick movement"
                            );
                        }

                        let _ = tx.send(InputEvent::Axis {
                            axis,
                            value: normalized,
                            centred,
                        });
                    }
                    EventSummary::Key(_, key, value) => {
                        let action = match value {
                            0 => ButtonAction::RELEASED,
                            1 => ButtonAction::PRESSED,
                            _ => ButtonAction::REPEATED,
                        };
                        debug!(button = ?ButtonLabel(key), ?action, "button event");
                        let _ = tx.send(InputEvent::Button { key, action });
                    }
                    EventSummary::Synchronization(..) | EventSummary::Misc(..) => {}
                    other => trace!(event = ?other, "unhandled event"),
                }
            }
        }
    });

    translate_controller_input_to_mouse_movements(rx)?;
    Ok(())
}

/// Wraps [`KeyCode`] so its `Debug` output reads as the bare constant name.
struct ButtonLabel(KeyCode);

impl std::fmt::Debug for ButtonLabel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            KeyCode::BTN_SOUTH => formatter.write_str("BTN_SOUTH (cross)"),
            KeyCode::BTN_EAST => formatter.write_str("BTN_EAST (circle)"),
            KeyCode::BTN_NORTH => formatter.write_str("BTN_NORTH (triangle)"),
            KeyCode::BTN_WEST => formatter.write_str("BTN_WEST (square)"),
            other => write!(formatter, "{other:?}"),
        }
    }
}

/// Per-axis calibration read from the kernel, used to normalize raw readings.
struct AxisTable {
    axes: HashMap<u16, AxisCalibration>,
}

struct AxisCalibration {
    minimum: i32,
    maximum: i32,
    deadzone: f32,
    unipolar: bool,
}

const UNIPOLAR_AXES: [AbsoluteAxisCode; 5] = [
    AbsoluteAxisCode::ABS_Z,
    AbsoluteAxisCode::ABS_RZ,
    AbsoluteAxisCode::ABS_GAS,
    AbsoluteAxisCode::ABS_BRAKE,
    AbsoluteAxisCode::ABS_THROTTLE,
];

impl AxisTable {
    fn probe(device: &evdev::Device) -> Self {
        let axes = match device.get_absinfo() {
            Ok(absinfo) => absinfo
                .map(|(axis, info)| (axis.0, AxisCalibration::new(axis, &info)))
                .collect(),
            Err(error) => {
                // Non-fatal: without calibration we still report raw values.
                warn!(%error, "could not read axis calibration; falling back to raw values");
                HashMap::new()
            }
        };

        Self { axes }
    }

    fn log_summary(&self) {
        if self.axes.is_empty() {
            return;
        }

        let mut summary = self
            .axes
            .iter()
            .map(|(code, calibration)| {
                format!(
                    "{:?}[{}..{}]",
                    AbsoluteAxisCode(*code),
                    calibration.minimum,
                    calibration.maximum
                )
            })
            .collect::<Vec<_>>();
        summary.sort();

        info!(axes = %summary.join(" "), "absolute axes detected");
    }

    fn normalize(&self, axis: AbsoluteAxisCode, value: i32) -> (f32, bool) {
        let Some(calibration) = self.axes.get(&axis.0) else {
            return (value as f32, value == 0);
        };

        let normalized = calibration.normalize(value);
        (normalized, normalized.abs() <= calibration.deadzone)
    }
}

impl AxisCalibration {
    fn new(axis: AbsoluteAxisCode, info: &AbsInfo) -> Self {
        let span = (info.maximum() - info.minimum()).max(1) as f32;
        let declared = (info.flat().max(info.fuzz()) as f32) / (span / 2.0);
        let deadzone = if declared > 0.0 {
            declared
        } else {
            FALLBACK_DEADZONE_RATIO
        };

        Self {
            minimum: info.minimum(),
            maximum: info.maximum(),
            deadzone,
            unipolar: UNIPOLAR_AXES.contains(&axis),
        }
    }

    fn normalize(&self, value: i32) -> f32 {
        let span = (self.maximum - self.minimum) as f32;
        if span <= 0.0 {
            return 0.0;
        }

        if self.unipolar {
            return ((value - self.minimum) as f32 / span).clamp(0.0, 1.0);
        }

        let centre = (self.minimum + self.maximum) as f32 / 2.0;
        ((value as f32 - centre) / (span / 2.0)).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A DualSense stick: 0..255, resting at 127, no declared flat/fuzz.
    fn stick() -> AxisCalibration {
        AxisCalibration::new(AbsoluteAxisCode::ABS_X, &AbsInfo::new(127, 0, 255, 0, 0, 1))
    }

    /// A DualSense trigger: 0..255, resting at 0.
    fn trigger() -> AxisCalibration {
        AxisCalibration::new(AbsoluteAxisCode::ABS_Z, &AbsInfo::new(0, 0, 255, 0, 0, 1))
    }

    #[test]
    fn stick_centre_is_zero_and_extremes_saturate() {
        let stick = stick();

        assert!(stick.normalize(127).abs() < 0.01);
        assert!(stick.normalize(255) > 0.99);
        assert!(stick.normalize(0) < -0.99);
    }

    #[test]
    fn resting_stick_is_inside_the_deadzone() {
        let stick = stick();

        // Idle drift around centre must not be reported as movement.
        assert!(stick.normalize(127).abs() <= stick.deadzone);
        assert!(stick.normalize(130).abs() <= stick.deadzone);
        // A real push must be.
        assert!(stick.normalize(240).abs() > stick.deadzone);
    }

    #[test]
    fn declared_flat_overrides_the_fallback_deadzone() {
        let generous = AxisCalibration::new(
            AbsoluteAxisCode::ABS_X,
            &AbsInfo::new(127, 0, 255, 0, 40, 1),
        );

        assert!(generous.deadzone > FALLBACK_DEADZONE_RATIO);
    }

    #[test]
    fn released_trigger_reads_zero_not_minus_one() {
        let trigger = trigger();

        // The bug this guards: centre-based normalization would report a
        // released trigger as -1.0, i.e. fully deflected.
        assert!(trigger.normalize(0).abs() < 0.01);
        assert!(trigger.normalize(255) > 0.99);
        assert!(trigger.normalize(128) > 0.4 && trigger.normalize(128) < 0.6);
    }

    #[test]
    fn released_trigger_is_inside_the_deadzone() {
        let trigger = trigger();

        assert!(trigger.normalize(0).abs() <= trigger.deadzone);
        assert!(trigger.normalize(255).abs() > trigger.deadzone);
    }

    #[test]
    fn hat_switch_maps_to_full_deflection() {
        let hat = AxisCalibration::new(
            AbsoluteAxisCode::ABS_HAT0X,
            &AbsInfo::new(0, -1, 1, 0, 0, 1),
        );

        assert!(hat.normalize(0).abs() < 0.01);
        assert!(hat.normalize(1) > 0.99);
        assert!(hat.normalize(-1) < -0.99);
    }

    #[test]
    fn degenerate_range_does_not_divide_by_zero() {
        let broken = AxisCalibration::new(AbsoluteAxisCode::ABS_X, &AbsInfo::new(0, 5, 5, 0, 0, 1));

        assert_eq!(broken.normalize(5), 0.0);
    }

    #[test]
    fn uncalibrated_axis_falls_back_to_raw_values() {
        let table = AxisTable {
            axes: HashMap::new(),
        };

        let (value, centred) = table.normalize(AbsoluteAxisCode::ABS_X, 42);
        assert_eq!(value, 42.0);
        assert!(!centred);

        let (_, centred) = table.normalize(AbsoluteAxisCode::ABS_X, 0);
        assert!(centred);
    }
}
