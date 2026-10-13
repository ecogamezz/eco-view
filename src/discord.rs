use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::metrics::Metrics;
use crate::settings::Settings;

pub fn spawn(shared: Arc<Mutex<Metrics>>, settings: Arc<Mutex<Settings>>) {
    thread::Builder::new()
        .name("eco-discord".into())
        .spawn(move || loop {
            thread::sleep(Duration::from_secs(2));
            let cfg = settings.lock().map(|s| s.clone()).unwrap_or_default();
            if !cfg.discord_enabled || cfg.discord_app_id.trim().is_empty() {
                continue;
            }
            let Ok(mut stream) = connect() else {
                continue;
            };
            if handshake(&mut stream, cfg.discord_app_id.trim()).is_err() {
                continue;
            }
            while settings
                .lock()
                .map(|s| s.discord_enabled && !s.discord_app_id.trim().is_empty())
                .unwrap_or(false)
            {
                let cfg = settings.lock().map(|s| s.clone()).unwrap_or_default();
                let snap = shared.lock().map(|m| m.snap.clone()).unwrap_or_default();
                let details = subst(&cfg.discord_details, &snap);
                let state = subst(&cfg.discord_state, &snap);
                let payload = presence_payload(
                    &cfg.discord_app_id,
                    &details,
                    &state,
                    &cfg.discord_large_image,
                    &cfg.discord_large_text,
                );
                if send(&mut stream, 1, &payload).is_err() {
                    break;
                }
                let mut buf = [0u8; 8];
                let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
                let _ = stream.read(&mut buf);
                thread::sleep(Duration::from_secs(15));
            }
        })
        .ok();
}

fn connect() -> std::io::Result<UnixStream> {
    let mut dirs = Vec::new();
    if let Ok(tmp) = std::env::var("TMPDIR") {
        dirs.push(PathBuf::from(tmp));
    }
    dirs.push(PathBuf::from("/tmp"));
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join("Library/Application Support/discord"));
    }
    for dir in dirs {
        for i in 0..10 {
            let path = dir.join(format!("discord-ipc-{i}"));
            if let Ok(stream) = UnixStream::connect(&path) {
                return Ok(stream);
            }
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "discord ipc",
    ))
}

fn handshake(stream: &mut UnixStream, client_id: &str) -> std::io::Result<()> {
    let json = format!(r#"{{"v":1,"client_id":"{}"}}"#, client_id.replace('"', ""));
    send(stream, 0, &json)?;
    let mut header = [0u8; 8];
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.read_exact(&mut header)?;
    let len = u32::from_le_bytes(header[4..8].try_into().unwrap_or([0; 4])) as usize;
    let mut body = vec![0u8; len.min(4096)];
    if !body.is_empty() {
        let _ = stream.read_exact(&mut body);
    }
    Ok(())
}

fn send(stream: &mut UnixStream, opcode: u32, json: &str) -> std::io::Result<()> {
    let bytes = json.as_bytes();
    let mut packet = Vec::with_capacity(8 + bytes.len());
    packet.extend_from_slice(&opcode.to_le_bytes());
    packet.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    packet.extend_from_slice(bytes);
    stream.write_all(&packet)
}

fn presence_payload(
    app_id: &str,
    details: &str,
    state: &str,
    large_image: &str,
    large_text: &str,
) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let image = if large_image.is_empty() {
        String::new()
    } else {
        format!(
            r#","large_image":"{}","large_text":"{}""#,
            esc(large_image),
            esc(large_text)
        )
    };
    let _ = app_id;
    format!(
        r#"{{"cmd":"SET_ACTIVITY","nonce":"{nonce}","args":{{"pid":{},"activity":{{"details":"{}","state":"{}","assets":{{{}}},"instance":false}}}}}}"#,
        std::process::id(),
        esc(details),
        esc(state),
        image.trim_start_matches(',')
    )
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn subst(template: &str, snap: &crate::metrics::Snapshot) -> String {
    let ram = crate::metrics::pct(snap.mem_used, snap.mem_total);
    let gpu = snap
        .gpu
        .load
        .map(|v| format!("{v:.0}%"))
        .unwrap_or_else(|| "n/a".into());
    let temp = snap
        .temperature
        .as_ref()
        .map(|t| format!("{:.0}°C", t.celsius))
        .unwrap_or_else(|| "n/a".into());
    template
        .replace("{cpu}", &format!("{:.0}%", snap.cpu_global))
        .replace("{ram}", &format!("{ram:.0}%"))
        .replace("{gpu}", &gpu)
        .replace("{temp}", &temp)
        .replace("{host}", &snap.hostname)
        .replace("{uptime}", &crate::format::uptime(snap.uptime_secs))
        .replace("{network_down}", &crate::format::bps(snap.net_down_bps))
        .replace("{network_up}", &crate::format::bps(snap.net_up_bps))
}

