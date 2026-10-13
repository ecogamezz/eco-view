use std::collections::VecDeque;
use std::ffi::OsStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use sysinfo::{Components, Disks, Networks, Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::audio::{self, AudioReading};
use crate::devices::{self, DeviceInfo};
use crate::gpu::{self, GpuReading};

const HISTORY: usize = 60;

#[derive(Clone, Debug)]
pub struct DiskInfo {
    pub name: String,
    pub mount: String,
    pub total: u64,
    pub available: u64,
}

#[derive(Clone, Debug)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
}

#[derive(Clone, Debug)]
pub struct TempInfo {
    pub celsius: f32,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct BatteryInfo {
    pub percent: f32,
    pub charging: bool,
    pub remaining: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub cpu_global: f32,
    pub cpu_per_core: Vec<f32>,
    pub mem_used: u64,
    pub mem_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub gpu: GpuReading,
    pub net_down_bps: f64,
    pub net_up_bps: f64,
    pub disks: Vec<DiskInfo>,
    pub battery: Option<BatteryInfo>,
    pub temperature: Option<TempInfo>,
    pub uptime_secs: u64,
    pub processes: Vec<ProcInfo>,
    pub devices: Vec<DeviceInfo>,
    pub audio: AudioReading,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            hostname: "Mac".into(),
            os: "macOS".into(),
            kernel: String::new(),
            cpu_brand: String::new(),
            cpu_cores: 0,
            cpu_global: 0.0,
            cpu_per_core: Vec::new(),
            mem_used: 0,
            mem_total: 0,
            swap_used: 0,
            swap_total: 0,
            gpu: GpuReading::default(),
            net_down_bps: 0.0,
            net_up_bps: 0.0,
            disks: Vec::new(),
            battery: None,
            temperature: None,
            uptime_secs: 0,
            processes: Vec::new(),
            devices: Vec::new(),
            audio: AudioReading::default(),
        }
    }
}

#[derive(Clone, Default)]
pub struct History {
    pub cpu: VecDeque<f32>,
    pub ram: VecDeque<f32>,
    pub gpu: VecDeque<f32>,
    pub net_down: VecDeque<f32>,
    pub net_up: VecDeque<f32>,
    pub audio: VecDeque<f32>,
}

impl History {
    fn push(slot: &mut VecDeque<f32>, value: f32) {
        if slot.len() >= HISTORY {
            slot.pop_front();
        }
        slot.push_back(value);
    }
}

#[derive(Clone, Default)]
pub struct Metrics {
    pub snap: Snapshot,
    pub history: History,
}

pub fn spawn(shared: Arc<Mutex<Metrics>>, refresh_ms: Arc<Mutex<u64>>) {
    thread::Builder::new()
        .name("eco-metrics".into())
        .spawn(move || worker(shared, refresh_ms))
        .expect("metrics thread");
}

fn worker(shared: Arc<Mutex<Metrics>>, refresh_ms: Arc<Mutex<u64>>) {
    let mut system = System::new_all();
    let mut disks = Disks::new_with_refreshed_list();
    let mut networks = Networks::new_with_refreshed_list();
    let mut components = Components::new_with_refreshed_list();
    thread::sleep(Duration::from_millis(200));
    let mut last_net = Instant::now();
    let mut last_rx: u64 = networks.values().map(|n| n.total_received()).sum();
    let mut last_tx: u64 = networks.values().map(|n| n.total_transmitted()).sum();
    // `Instant - Duration` panics on underflow; uptime may be shorter than 30s.
    let mut last_devices = Instant::now()
        .checked_sub(Duration::from_secs(30))
        .unwrap_or_else(Instant::now);
    let mut devices = Vec::new();
    let mut tick: u64 = 0;

    loop {
        let wait = refresh_ms.lock().map(|v| *v).unwrap_or(1000).clamp(250, 5000);
        thread::sleep(Duration::from_millis(wait));
        tick += 1;

        system.refresh_cpu_all();
        system.refresh_memory();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::everything(),
        );

        if tick.is_multiple_of(2) {
            disks.refresh(true);
            components.refresh(true);
        }
        networks.refresh(true);
        let now = Instant::now();
        let dt = now.duration_since(last_net).as_secs_f64().max(0.001);
        let rx: u64 = networks.values().map(|n| n.total_received()).sum();
        let tx: u64 = networks.values().map(|n| n.total_transmitted()).sum();
        let net_down = (rx.saturating_sub(last_rx) as f64) / dt;
        let net_up = (tx.saturating_sub(last_tx) as f64) / dt;
        last_rx = rx;
        last_tx = tx;
        last_net = now;

        if last_devices.elapsed() > Duration::from_secs(12) {
            devices = devices::inventory();
            last_devices = Instant::now();
        }

        let gpu = if tick.is_multiple_of(2) {
            gpu::read_gpu()
        } else {
            shared
                .lock()
                .map(|m| m.snap.gpu.clone())
                .unwrap_or_default()
        };
        let audio = audio::read_output();
        let battery = read_battery();
        let temperature = read_temp(&components);

        let cpus = system.cpus();
        let cpu_per_core: Vec<f32> = cpus.iter().map(|c| c.cpu_usage()).collect();
        let cpu_global = system.global_cpu_usage();
        let cpu_brand = cpus
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_default();

        let mut processes: Vec<ProcInfo> = system
            .processes()
            .iter()
            .map(|(pid, proc)| ProcInfo {
                pid: pid_u32(*pid),
                name: os_name(proc.name()),
                cpu: proc.cpu_usage(),
                memory: proc.memory(),
            })
            .filter(|p| !p.name.is_empty())
            .collect();
        processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
        processes.truncate(24);

        let disk_info: Vec<DiskInfo> = disks
            .iter()
            .filter(|d| !d.mount_point().starts_with("/System/Volumes"))
            .map(|d| DiskInfo {
                name: os_name(d.name()),
                mount: d.mount_point().to_string_lossy().into_owned(),
                total: d.total_space(),
                available: d.available_space(),
            })
            .collect();

        let snap = Snapshot {
            hostname: System::host_name().unwrap_or_else(|| "Mac".into()),
            os: System::long_os_version()
                .or_else(System::name)
                .unwrap_or_else(|| "macOS".into()),
            kernel: System::kernel_version().unwrap_or_default(),
            cpu_brand,
            cpu_cores: cpus.len(),
            cpu_global,
            cpu_per_core,
            mem_used: system.used_memory(),
            mem_total: system.total_memory(),
            swap_used: system.used_swap(),
            swap_total: system.total_swap(),
            gpu,
            net_down_bps: net_down,
            net_up_bps: net_up,
            disks: disk_info,
            battery,
            temperature,
            uptime_secs: System::uptime(),
            processes,
            devices: devices.clone(),
            audio,
        };

        if let Ok(mut guard) = shared.lock() {
            History::push(&mut guard.history.cpu, snap.cpu_global);
            let ram_pct = pct(snap.mem_used, snap.mem_total);
            History::push(&mut guard.history.ram, ram_pct);
            History::push(
                &mut guard.history.gpu,
                snap.gpu.load.unwrap_or(0.0),
            );
            History::push(&mut guard.history.net_down, snap.net_down_bps as f32);
            History::push(&mut guard.history.net_up, snap.net_up_bps as f32);
            History::push(
                &mut guard.history.audio,
                snap.audio.volume.unwrap_or(0.0) * 100.0,
            );
            guard.snap = snap;
        }
    }
}

fn pid_u32(pid: Pid) -> u32 {
    pid.as_u32()
}

fn os_name(name: &OsStr) -> String {
    name.to_string_lossy().into_owned()
}

pub fn pct(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (used as f64 / total as f64 * 100.0) as f32
    }
}

fn read_temp(components: &Components) -> Option<TempInfo> {
    let preference = ["TC0P", "TXCX", "TXCx", "TG0P", "TB0T", "CPU", "GPU"];
    for needle in preference {
        for component in components {
            let label = component.label();
            if label.contains(needle)
                && let Some(celsius) = component.temperature() {
                    return Some(TempInfo {
                        celsius,
                        source: label.to_string(),
                    });
                }
        }
    }
    components.iter().find_map(|component| {
        component.temperature().map(|celsius| TempInfo {
            celsius,
            source: component.label().to_string(),
        })
    })
}

fn read_battery() -> Option<BatteryInfo> {
    let output = std::process::Command::new("pmset")
        .args(["-g", "batt"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut percent = None;
    let mut charging = false;
    let mut remaining = None;
    for line in text.lines() {
        if line.contains("InternalBattery") || line.contains('%') {
            if let Some(idx) = line.find('%') {
                // Walk back over digit chars from the '%' rather than slicing at
                // a byte offset, which can land mid-codepoint and panic.
                let digits: String = line[..idx]
                    .chars()
                    .rev()
                    .take_while(char::is_ascii_digit)
                    .collect();
                percent = digits.chars().rev().collect::<String>().parse().ok();
            }
            // "discharging" contains "charging", so match whole words. A
            // battery that is merely present is not charging.
            let mut words = line.split(|c: char| !c.is_ascii_alphabetic());
            charging = words.any(|w| w == "charging" || w == "charged");
            // "60%; discharging; 0:54 remaining present: true" -> "0:54"
            if let Some((_, rest)) = line.split_once(';')
                && let Some(time) = rest.split_whitespace().find(|w| w.contains(':')) {
                    remaining = Some(time.trim_end_matches(':').to_string());
                }
        }
        if line.contains("AC Power") {
            charging = true;
        }
    }
    Some(BatteryInfo {
        percent: percent?,
        charging,
        remaining,
    })
}
