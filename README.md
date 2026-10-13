# eco view

**eco view** is a macOS desktop system monitor for Intel Macs. It is built in Rust with eframe/egui and the OpenGL (`glow`) renderer, which remains the reliable GPU path on x86_64 MacBooks.

## Features

- Live CPU load and per-core activity, memory and swap, GPU engine load and dedicated/system GPU memory when IOKit `IOAccelerator` publishes those statistics, network throughput, storage usage, battery state, uptime, and top resource-using processes.
- Native Core Audio output volume and mute for the default render device. eco view does not record audio or open the microphone. macOS does not expose a WASAPI-style peak meter without an audio tap, so the meter shows the hardware output volume.
- Short rolling charts, capacity bars, network rates, health thresholds, local system details, and a responsive dashboard that keeps tiles on a 48-column grid.
- Neon arcade presentation with animated ambient light rings, moving particles, and softly pulsing card borders. Effect intensity can be adjusted or disabled in Settings.
- Vector-drawn icons for every navigation and dashboard category.
- Separate overview, performance, process, storage, and settings pages.
- USB, Bluetooth, HID, and likely game-controller inventory from IOKit. Controller-like names get a dedicated controller mark. Device probes run on a slower background cadence than CPU/memory.
- SMC thermal readings through `sysinfo` components when the firmware exposes them (CPU proximity `TC0P`, PECI `TXCX`, GPU `TG0P`, battery `TB0T` on Intel). The source is labeled; missing sensors show as unavailable.
- Six selectable animated particle styles: Arcade Sparks, Ember Drift, Neon Snow, Matrix Rain, Orbit Rings, and Aurora Waves.
- A 48-column saved dashboard grid. Enable **Edit layout** to move a widget from its top-left handle or resize it in grid increments from its bottom-right handle. Overlapping layouts are rejected; settings persist automatically. CPU-core and network heatmap modes are included.
- Optional custom application/window icon loaded from PNG, JPG/JPEG, BMP, or ICO, with a restore-to-default control.
- Optional toast, warning banner, and native macOS notification feedback (`osascript` notifications).
- Adjustable click, accent, and warning sounds (`afplay` of system sounds) with a volume control; optional Login Item via a per-user LaunchAgent that points at the running executable.
- Optional Discord Rich Presence over Discord's local UNIX IPC socket, with configurable details/state/assets and live placeholders such as `{cpu}`, `{ram}`, `{gpu}`, `{temp}`, `{host}`, `{uptime}`, `{network_down}`, and `{network_up}`. A Discord application ID and image keys from the Discord Developer Portal are required.
- Saved settings for dark/light theme, accent colors, text scale, refresh rate, warning thresholds, card visibility and grid placement/size, heatmaps, particles, notifications, sounds, icon, autostart, Discord presence, and the raccoon companion.
- Settings are stored as JSON in `~/Library/Application Support/eco-view/settings.json` and saved automatically after edits.
- Sensor collection runs on background threads; dashboard painting stays separate from the system polling cadence.

## Raccoon companion

A small circular chip floats above the UI. It is its own overlay: it does not participate in the dashboard grid, navigation layout, or page margins, and you can drag it anywhere. Its resting spot is saved, so it comes back where you left it.

Click the chip to toggle a 3D low-poly raccoon NPC. It walks the dashboard, looks around, jumps onto metric cards, sleeps when the machine is idle, panics when load spikes, and cheers when load drops. The companion is drawn on a foreground layer so it cannot shove other widgets around.

The raccoon is real 3D geometry, not a sprite: it is assembled from ellipsoids and triangles at runtime, then back-face culled, depth-sorted, Lambert-shaded, and perspective-projected each frame, with the head turning on a yaw/pitch rig and the limbs swinging off a walk phase.

Its reactions: idle glancing, wandering between cards, jumping onto them, sleeping when the machine is idle, an alert dash with a `!` when load spikes, and a cheer hop when load drops. The mood decay is frame-rate independent, so a 120Hz display behaves the same as 60Hz.

## Build on Intel macOS

Requirements: an Intel Mac, Xcode Command Line Tools, and Rust (`rustup`).

```zsh
chmod +x ./build.sh
./build.sh
```

That produces `outputs/eco-view.app` targeting `x86_64-apple-darwin`. For a faster debug build:

```zsh
./build.sh debug
open outputs/eco-view.app
```

The first run needs internet access to fetch crates.

## Data and privacy

All monitoring happens locally. eco view does not upload telemetry. Network rates are calculated from interface byte counters on this computer. Audio uses the default output device volume only; no audio samples are copied or stored. Preferences are local JSON under the current macOS user.

## Tests

```zsh
cargo test
```

Covers the layout grid (overlap rejection, clamping, and the no-change detection that keeps settings from being rewritten on every frame of a drag) and the `pmset` battery parsing that the UI depends on.

## Project layout

- `src/main.rs` — native application entry point.
- `src/app.rs` — views, layout, theme wiring, and persistence.
- `src/metrics.rs` — background system sensor collection.
- `src/gpu.rs` — IOKit `IOAccelerator` GPU reader.
- `src/audio.rs` — Core Audio default-output worker.
- `src/autostart.rs` — per-user LaunchAgent login item.
- `src/discord.rs` — Discord Rich Presence UNIX IPC worker.
- `src/notifications.rs` and `src/sound.rs` — macOS notification and system-sound workers.
- `src/companion.rs` — floating toggle chip and 3D raccoon NPC.
- `build.sh` — Intel macOS `.app` bundle build.
- `tests/integration.rs` — layout grid and battery parsing regression tests.
