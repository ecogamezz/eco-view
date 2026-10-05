# eco view

**eco view** is a Windows desktop system monitor built in Rust. It uses eframe/egui with the OpenGL renderer for GPU-accelerated dashboard drawing when a compatible graphics driver is available.

## Features

- Live CPU load and per-core activity, memory and swap, GPU engine load and dedicated GPU memory when Windows exposes those performance counters, network throughput, storage usage, battery state, uptime, and top resource-using processes.
- Native Windows audio output peak meter. It reads the default render endpoint's peak level; it does not record audio or open the microphone.
- Short rolling charts, capacity bars, network rates, health thresholds, local system details, and a responsive dashboard that keeps every tile within the window.
- Neon arcade presentation with animated ambient light rings, moving particles, twinkling trails, and softly pulsing card borders. Effect intensity can be adjusted or disabled in Settings.
- Crisp vector-drawn icons for every navigation and dashboard category, independent of the installed symbol font.
- Separate overview, performance, process, storage, and settings pages.
- USB, Bluetooth, HID, and likely game-controller inventory from Windows Plug and Play. Controller devices get a dedicated controller icon. Device and thermal probes run on a separate, low-frequency background worker.
- ACPI thermal-zone temperature when exposed by the PC firmware. This can be a motherboard zone rather than the CPU package sensor; eco view labels the source and reports unavailable when Windows has no reading.
- Six selectable animated particle styles: Arcade Sparks, Ember Drift, Neon Snow, Matrix Rain, Orbit Rings, and Aurora Waves.
- A 48-column saved dashboard grid for both summary tiles and cards. Enable **Raster bearbeiten** to move a widget with its top-left handle or resize it in grid increments with its bottom-right handle. Overlapping layouts are rejected; settings persist automatically. CPU-core and network heatmap modes are included.
- Optional custom application/window icon loaded from PNG, JPG/JPEG, BMP, or ICO, with a one-click restore to the default icon.
- Optional toast, snackbar, warning badge, dashboard banner, warning sheet, and native Windows taskbar notification feedback.
- Adjustable click, accent, and warning sounds with a volume control; optional HKCU Run registry autostart that uses the running executable's path.
- Optional Discord Rich Presence over Discord's local IPC pipe, with configurable details/state/assets and live placeholders such as `{cpu}`, `{ram}`, `{gpu}`, `{temp}`, `{host}`, `{uptime}`, `{network_down}`, and `{network_up}`. A Discord application ID and image keys from the user's Discord Developer Portal are required.
- Saved settings for dark/light theme, multiple accent and surface colors, text scale/effects, font, slider and border styles, refresh rate, warning thresholds, card visibility and grid placement/size, selected heatmaps, particles, notifications, sounds, icon, autostart, and Discord presence.
- Settings are stored as JSON in `%APPDATA%\eco-view\settings.json` and saved automatically after edits.
- Sensor collection runs on background threads; dashboard painting stays separate from the system polling cadence.

GPU engine and memory counters are provided by Windows Performance Counters. If a driver does not publish a counter, eco view clearly shows that reading as unavailable instead of estimating a value. CPU, memory, network, process, and storage values are read through the cross-platform `sysinfo` crate. The dashboard uses eframe/egui with OpenGL for GPU-accelerated drawing.

## Build on Windows

Requirements: Windows 10/11 x64, PowerShell, and `winget`. Run `build.ps1` from this folder. It checks for Rust and the Microsoft C++ Build Tools/Windows SDK, installs missing prerequisites where possible, downloads Rust dependencies through Cargo, builds an optimized MSVC executable with a statically linked Visual C++ runtime, then copies it to `outputs\eco-view.exe`. If that executable is currently open and locked by Windows, the script leaves it alone and writes the new build under a timestamped name in `outputs`.

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\build.ps1
```

The first run needs internet access to install prerequisites and fetch crates. Visual Studio Build Tools may show an administrator approval prompt. To only build when the C++ toolchain is already present, use `-SkipBuildToolsInstall`.

For a faster debug build, use `.\build.ps1 -DebugBuild`.

## Data and privacy

All monitoring happens locally. eco view does not upload telemetry. Network rates are calculated from interface byte counters on the computer. Audio uses the Windows endpoint peak meter only; no audio samples are copied or stored. Preferences are local JSON under the current Windows user profile.

## Project layout

- `src/main.rs` — native application entry point.
- `src/app.rs` — views, layout, theme, settings, and persistence.
- `src/metrics.rs` — background system sensor collection.
- `src/gpu.rs` — Windows GPU Performance Counter reader.
- `src/audio.rs` — Windows output meter worker.
- `src/autostart.rs` — per-user Windows Run registry integration.
- `src/discord.rs` — configurable Discord Rich Presence IPC worker.
- `src/notifications.rs` and `src/sound.rs` — native Windows notification and generated UI sound workers.
- `build.ps1` — prerequisite bootstrap and reproducible Windows build.
