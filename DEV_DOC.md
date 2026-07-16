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
