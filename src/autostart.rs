use std::fs;
use std::path::PathBuf;
use std::process::Command;

const LABEL: &str = "com.ecoview.monitor";

fn plist_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(format!("Library/LaunchAgents/{LABEL}.plist"))
}

fn exe_path() -> Option<PathBuf> {
    std::env::current_exe().ok()?.canonicalize().ok()
}

pub fn is_enabled() -> bool {
    plist_path().exists()
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    let path = plist_path();
    if !on {
        let _ = Command::new("launchctl").args(["unload", "-w"]).arg(&path).output();
        let _ = fs::remove_file(&path);
        return Ok(());
    }
    let exe = exe_path().ok_or("could not resolve executable path")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>LimitLoadToSessionType</key>
  <string>Aqua</string>
</dict>
</plist>
"#,
        exe.display()
    );
    fs::write(&path, body).map_err(|e| e.to_string())?;
    let _ = Command::new("launchctl")
        .args(["unload", "-w"])
        .arg(&path)
        .output();
    let status = Command::new("launchctl")
        .args(["load", "-w"])
        .arg(&path)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("launchctl load failed".into())
    }
}
