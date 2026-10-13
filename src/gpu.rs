use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct GpuReading {
    pub name: Option<String>,
    pub load: Option<f32>,
    pub mem_used: Option<u64>,
    pub mem_total: Option<u64>,
}

pub fn read_gpu() -> GpuReading {
    let output = Command::new("ioreg")
        .args(["-r", "-d", "1", "-w", "0", "-c", "IOAccelerator"])
        .output();
    let Ok(output) = output else {
        return GpuReading::default();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut reading = parse_ioreg(&text);
    reading.name = reading.name.map(|n| friendly_name(&n));
    reading
}

/// IOAccelerator often only names its driver bundle
/// ("com.apple.kext.AMDRadeonX4000"); swap that for the PCI device's model
/// ("Radeon Pro 555X") when one matches the vendor.
fn friendly_name(raw: &str) -> String {
    static MODELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let models = MODELS.get_or_init(|| {
        let Ok(out) = Command::new("ioreg").args(["-rd1", "-c", "IOPCIDevice", "-w0"]).output() else {
            return Vec::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| {
                let l = l.trim();
                let rest = l.strip_prefix("\"model\" = <\"")?;
                Some(rest.trim_end_matches("\">").to_string())
            })
            .collect()
    });
    let lower = raw.to_lowercase();
    let vendor = if lower.contains("amd") || lower.contains("radeon") || lower.contains("ati") {
        "radeon"
    } else if lower.contains("intel") {
        "intel"
    } else if lower.contains("nvidia") || lower.contains("geforce") {
        "geforce"
    } else {
        return raw.to_string();
    };
    models
        .iter()
        .find(|m| m.to_lowercase().contains(vendor))
        .cloned()
        .unwrap_or_else(|| raw.trim_start_matches("com.apple.kext.").to_string())
}

fn parse_ioreg(text: &str) -> GpuReading {
    let mut reading = GpuReading::default();
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(name) = string_value(line, "model") {
            reading.name = Some(name);
        }
        if let Some(name) = string_value(line, "CFBundleIdentifier")
            && reading.name.is_none()
        {
            reading.name = Some(name);
        }
        if let Some(v) = int_value(line, "Device Utilization %") {
            reading.load = Some(v as f32);
        } else if reading.load.is_none() {
            if let Some(v) = int_value(line, "Renderer Utilization %") {
                reading.load = Some(v as f32);
            } else if let Some(v) = int_value(line, "GPU Core Utilization") {
                reading.load = Some(v as f32 / 10_000_000.0);
            }
        }
        // This machine publishes `inUseVidMemoryBytes` / `inUseSysMemoryBytes`;
        // older drivers use `vramUsedBytes` / `In use system memory`.
        if let Some(v) = int_value(line, "vramUsedBytes") {
            reading.mem_used = Some(v);
        } else if let Some(v) = int_value(line, "inUseVidMemoryBytes") {
            reading.mem_used = Some(v);
        } else if let Some(v) = int_value(line, "inUseSysMemoryBytes") {
            reading.mem_used = Some(v);
        } else if let Some(v) = int_value(line, "In use system memory") {
            reading.mem_used = Some(v);
        }
        if let Some(v) = int_value(line, "vramFreeBytes") {
            let used = reading.mem_used.unwrap_or(0);
            reading.mem_total = Some(used.saturating_add(v));
        } else if let Some(v) = int_value(line, "Alloc system memory") {
            reading.mem_total = Some(v);
        }
    }
    reading
}

fn string_value(line: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let idx = line.find(&needle)?;
    let rest = &line[idx + needle.len()..];
    let rest = rest.split_once('=')?.1.trim();
    let rest = rest.trim_matches(|c| c == '"' || c == ',' || c == ' ');
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

fn int_value(line: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\"");
    let idx = line.find(&needle)?;
    let rest = &line[idx + needle.len()..];
    let rest = rest.split_once('=')?.1.trim();
    let num: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    num.parse().ok()
}
