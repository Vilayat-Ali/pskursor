use std::{
    fs,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, InputEvent, KeyCode, UinputAbsSetup,
    uinput::VirtualDevice,
};

const VIRTUAL_DEVICE_NAME: &str = "pskursor test pad";

fn uinput_available() -> bool {
    fs::OpenOptions::new()
        .write(true)
        .open("/dev/uinput")
        .is_ok()
}

fn build_virtual_gamepad() -> VirtualDevice {
    // 0..255 with a centre of 127, matching a DualSense stick.
    let axis = AbsInfo::new(127, 0, 255, 0, 0, 1);

    let mut keys = AttributeSet::<KeyCode>::new();
    keys.insert(KeyCode::BTN_SOUTH);

    VirtualDevice::builder()
        .expect("open /dev/uinput")
        .name(VIRTUAL_DEVICE_NAME)
        .with_keys(&keys)
        .expect("register keys")
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, axis))
        .expect("register ABS_X")
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, axis))
        .expect("register ABS_Y")
        .build()
        .expect("build virtual device")
}

/// Find the `/dev/input/eventN` path the kernel assigned to our virtual device.
fn virtual_device_path() -> Option<String> {
    evdev::enumerate()
        .find(|(_, device)| device.name() == Some(VIRTUAL_DEVICE_NAME))
        .map(|(path, _)| path.to_string_lossy().to_string())
}

#[test]
fn daemon_reports_stick_movement_and_button_presses() {
    if !uinput_available() {
        eprintln!("skipping: /dev/uinput is not writable");
        return;
    }

    let mut virtual_gamepad = build_virtual_gamepad();

    // Give udev a moment to create the event node.
    let mut evdev_path = None;
    for _ in 0..50 {
        if let Some(path) = virtual_device_path() {
            evdev_path = Some(path);
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let evdev_path = evdev_path.expect("virtual device did not appear under /dev/input");

    // Point the daemon at the virtual device via a scratch DEVAYNC config.
    let config_dir = std::env::temp_dir().join("pskursor-event-stream-test");
    fs::create_dir_all(&config_dir).expect("create temp config dir");
    let config_path = config_dir.join("devaync.json");
    fs::write(
        &config_path,
        format!(
            r#"{{"vendor_id":0,"product_id":0,"device_name":"{VIRTUAL_DEVICE_NAME}",
                 "mac_addr":null,"evdev_path":"{evdev_path}",
                 "supports_cursor_control":true}}"#
        ),
    )
    .expect("write temp config");

    let mut daemon = Command::new(env!("CARGO_BIN_EXE_pskursor"))
        .env("PSKURSOR_CONFIG", &config_path)
        // Default filter must already surface these events; do not set RUST_LOG.
        .env_remove("RUST_LOG")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");

    let stderr = BufReader::new(daemon.stderr.take().expect("capture stderr"));

    // Emit input from a separate thread so reading the daemon's log cannot
    // deadlock against the writes.
    let emitter = thread::spawn(move || {
        thread::sleep(Duration::from_millis(400));

        for _ in 0..3 {
            // Push ABS_X well past the deadzone, then release to centre.
            let _ = virtual_gamepad.emit(&[
                InputEvent::new(evdev::EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_X.0, 250),
                InputEvent::new(evdev::EventType::KEY.0, KeyCode::BTN_SOUTH.0, 1),
            ]);
            thread::sleep(Duration::from_millis(60));

            let _ = virtual_gamepad.emit(&[
                InputEvent::new(evdev::EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_X.0, 127),
                InputEvent::new(evdev::EventType::KEY.0, KeyCode::BTN_SOUTH.0, 0),
            ]);
            thread::sleep(Duration::from_millis(60));
        }
    });

    let mut saw_movement = false;
    let mut saw_button = false;
    let mut transcript = Vec::new();

    for line in stderr.lines().map_while(std::result::Result::ok) {
        transcript.push(line.clone());

        if line.contains("joystick movement") && line.contains("ABS_X") {
            saw_movement = true;
        }
        if line.contains("button event") && line.contains("BTN_SOUTH") {
            saw_button = true;
        }
        if saw_movement && saw_button {
            break;
        }
        if transcript.len() > 200 {
            break;
        }
    }

    let _ = daemon.kill();
    let _ = daemon.wait();
    let _ = emitter.join();
    let _ = fs::remove_file(&config_path);

    // Visible with `cargo test -- --nocapture`.
    for line in &transcript {
        eprintln!("daemon| {line}");
    }

    assert!(
        saw_movement,
        "daemon did not log stick movement at the default log level.\n--- log ---\n{}",
        transcript.join("\n")
    );
    assert!(
        saw_button,
        "daemon did not log the button press at the default log level.\n--- log ---\n{}",
        transcript.join("\n")
    );
}
