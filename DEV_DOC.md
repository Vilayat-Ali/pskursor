# Dev Doc

This document contains detailed information needed for debugging and implementation of this project.

## Reading Device Information in Linux

In Linux, everything is treated as a file—including your mouse, keyboard, and trackpad. When you interact with a device, the Linux kernel captures that action and turns it into a data stream that applications can read.

### Device Catalog: `/proc/bus/input/devices`

This virtual file acts as a live inventory of every input device connected to your system. Running `cat /proc/bus/input/devices` displays distinct blocks of text for each device. 

Here is what a typical entry looks like:

```ini
I: Bus=0003 Vendor=04d9 Product=a09f Version=0110
N: Name="E-Signal Kreo Hawk"
P: Phys=usb-0000:00:14.0-7.4/input0
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/.../input/input16
H: Handlers=mouse2 event12 
B: EV=17
B: REL=1943

```

* **`I:` (Identity):** Connection type and hardware IDs. `Bus=0003` means USB (`0011` is an internal keyboard/mouse bus, `0019` is a virtual platform bus). It also contains the exact hex codes for **Vendor** and **Product**.
* **`N:` (Name):** The human-readable name string assigned to the device. **Our Rust backend scans this line to dynamically find the correct hardware.**
* **`P:` & `S:` (Physical & System Paths):** Internal system mappings showing the exact port or motherboard lane the device uses.
* **`H:` (Handlers):** **Crucial.** Lists the active files under `/dev/input/` receiving this device's data. For example, `event12` means the real-time stream is accessible at `/dev/input/event12`.
* **`B:` (Bitmasks / Capabilities):** Flags defining what the device can do. `REL` indicates relative axis tracking (like standard mouse movements), while `KEY` indicates button capabilities.

> ⚠️ **Note on Composite Hardware:** Gaming mice and modern mechanical keyboards frequently appear **multiple times** in this inventory. A single physical gaming mouse will often expose one entry for structural cursor movement (`event12`), a second entry masking as a virtual keyboard to fire side-button macros (`event13`), and a third endpoint for managing RGB lighting or configuration profiles (`event14`).

---

### Active Subsystems: `/proc/bus/input/handlers`

While the `devices` file tells you *what* is plugged in, running `cat /proc/bus/input/handlers` tells you *which drivers* are currently processing those inputs.

This file lists the system kernel modules waiting to react to incoming events. The primary handlers include:

* **`evdev` (Event Device):** The modern Linux input standard. It passes raw, completely unfiltered data packets (pixel deltas, precise click states, scroll ticks) straight to userspace. **Our Rust project targets this handler.**
* **`kbd` (Keyboard):** The traditional core driver that processes typing scan codes into standard text for terminals and console layouts.
* **`mousedev` (Mouse Emulator):** A legacy compatibility layer that pools all coordinates from connected pointers into a single combined layout (`/dev/input/mice`).
* **`rfkill` (Radio Frequency Switch):** A specialized module that listens to hardware toggles or laptop Fn keys to instantly enable or disable wireless radios (Wi-Fi, Bluetooth).
* **`sysrq` (Magic SysRq):** A low-level filter that intercepts diagnostic emergency commands (like safely unmounting filesystems or forcing system reboots) even if your graphical environment completely crashes.

---

## Debugging: "the daemon prints nothing when I move the sticks"

Three distinct causes, in the order worth checking.

### 1. Log level

Controller events are emitted at `DEBUG`. A subscriber initialised with an
`info` default filter drops them, so the daemon looks idle even while it is
reading events correctly. The default filter is therefore `pskursor=debug`, not
`info`. `RUST_LOG` overrides it.

Note also that `tracing_subscriber::fmt()` writes to **stdout** by default;
pskursor sends diagnostics to **stderr** so stdout stays clean under a pipe.

### 2. The wrong event node was activated

A DualSense exposes three nodes (see the table in `README.md`). Only the plain
gamepad node has sticks; the Touchpad node also reports `ABS_X`/`ABS_Y`, so an
axes-only capability check will happily nominate it.

The discriminator is `BTN_SOUTH` (aka `BTN_GAMEPAD`), the kernel's canonical
marker for a gamepad action pad. `device::missing_gamepad_capabilities` requires
`ABS_X` + `ABS_Y` **and** a gamepad button, and reports which check failed.

Event node numbers are not stable across reconnects, so the daemon re-validates
capabilities on startup instead of trusting the persisted flag.

### 3. Permissions

`/dev/input/event*` is `root:input` `0660`. Without membership of the `input`
group `Device::open` fails with `PermissionDenied`, which pskursor reports with
the `usermod` invocation needed to fix it.

---

## Axis normalization

Raw readings are normalized for logging and for the eventual cursor mapping:

* **Sticks and hats** rest at their centre and map to `-1.0..=1.0`.
* **Triggers** (`ABS_Z`, `ABS_RZ`, `ABS_GAS`, `ABS_BRAKE`, `ABS_THROTTLE`) rest
  at their *minimum* and map to `0.0..=1.0`. Centre-based normalization would
  report a released trigger as `-1.0`, i.e. fully deflected. Trigger-vs-stick
  cannot be inferred from the range — a DualSense reports `0..255` for both — so
  it is decided by axis code.

The deadzone uses the kernel's `flat`/`fuzz` values when the device declares
them, and falls back to 12% of the half-range otherwise (a DualSense reports
`flat = 0`). Readings inside the deadzone are logged at `TRACE`, not `DEBUG`, so
idle stick drift does not flood the log.

---

## Testing against hardware you do not have

`tests/event_stream.rs` builds a **uinput virtual gamepad**, activates it through
the normal DEVAYNC flow, emits stick and button events, and asserts the daemon
logs them at the *default* log level — which is what regression-guards the bug
described above. It skips itself when `/dev/uinput` is not writable.
