pub fn bytes(n: u64) -> String {
    const KIB: f64 = 1024.0;
    let n = n as f64;
    if n < KIB {
        format!("{n:.0} B")
    } else if n < KIB * KIB {
        format!("{:.1} KB", n / KIB)
    } else if n < KIB * KIB * KIB {
        format!("{:.1} MB", n / (KIB * KIB))
    } else {
        format!("{:.2} GB", n / (KIB * KIB * KIB))
    }
}

pub fn bps(n: f64) -> String {
    const KIB: f64 = 1024.0;
    if n < KIB {
        format!("{n:.0} B/s")
    } else if n < KIB * KIB {
        format!("{:.1} KB/s", n / KIB)
    } else {
        format!("{:.1} MB/s", n / (KIB * KIB))
    }
}

pub fn uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let mins = (secs % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else {
        format!("{hours}h {mins}m")
    }
}

/// "Mon 14 Oct, 09:12" from a Unix timestamp.
pub fn boot_time(epoch: u64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_opt(epoch as i64, 0).single() {
        Some(t) if epoch > 0 => t.format("%a %-d %b, %H:%M").to_string(),
        _ => "—".into(),
    }
}
