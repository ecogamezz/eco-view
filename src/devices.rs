use std::process::Command;

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub name: String,
    pub kind: DeviceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Usb,
    Bluetooth,
    Hid,
    Controller,
}

pub fn inventory() -> Vec<DeviceInfo> {
    let mut devices = Vec::new();
    collect_ioreg(
        &mut devices,
        &["-p", "IOUSB", "-l", "-w", "0"],
        DeviceKind::Usb,
        "USB Product Name",
    );
    collect_ioreg(
        &mut devices,
        &["-r", "-c", "IOBluetoothDevice", "-w", "0"],
        DeviceKind::Bluetooth,
        "Name",
    );
    collect_ioreg(
        &mut devices,
        &["-r", "-c", "IOHIDDevice", "-w", "0"],
        DeviceKind::Hid,
        "Product",
    );
    devices.sort_by_key(|d| d.name.to_lowercase());
    devices.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    for device in &mut devices {
        if looks_like_controller(&device.name) {
            device.kind = DeviceKind::Controller;
        }
    }
    devices
}

fn collect_ioreg(out: &mut Vec<DeviceInfo>, args: &[&str], kind: DeviceKind, key: &str) {
    let Ok(output) = Command::new("ioreg").args(args).output() else {
        return;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let needle = format!("\"{key}\"");
    for line in text.lines() {
        let Some(idx) = line.find(&needle) else {
            continue;
        };
        let Some((_, rhs)) = line[idx..].split_once('=') else {
            continue;
        };
        let name = rhs
            .trim()
            .trim_matches(|c| c == '"' || c == ',' || c == ' ')
            .trim();
        if name.is_empty() || name == "null" || name.len() > 80 {
            continue;
        }
        if name.contains("Hub") && kind == DeviceKind::Usb {
            continue;
        }
        out.push(DeviceInfo {
            name: name.to_string(),
            kind,
        });
    }
}

fn looks_like_controller(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Plenty of internal hardware is a "controller" (the T2, USB hosts...).
    let hardware = ["apple", "t2", "usb", "host", "hub", "bluetooth", "thunderbolt", "keyboard", "trackpad"];
    let generic = n.contains("controller") && !hardware.iter().any(|h| n.contains(h));
    generic
        || n.contains("gamepad")
        || n.contains("joystick")
        || n.contains("xbox")
        || n.contains("dualshock")
        || n.contains("dualsense")
        || n.contains("playstation")
        || n.contains("switch pro")
}
