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
    parse_ioreg(&text)
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
