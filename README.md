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

A small circular chip on the dashboard toggles him. The chip is a foreground `egui::Area` that lives entirely outside the grid and page margins, so it can never displace a card.

Click it and he leaves the app: he is drawn in a **separate always-on-top, transparent window** that floats above every other application. That window is deliberately small and follows him rather than covering the screen, which is what keeps the rest of the desktop usable — clicks that miss it fall through to whatever is underneath, so there are no click-through regions to tune.

You can pick him up and throw him anywhere. Dragging carries his body, and releasing hands him the pointer's velocity, so he flies, spins, bounces off the floor and the sides of the screen, and lands hard enough to go dizzy.

### Sprite sheets

There is no 3D mesh. He is a pixel-art sprite sheet, generated once at startup into a single atlas texture with one cell per animation frame — 263 frames across 34 clips. Each frame is described by a small pose vector (limb phases, body bob, squash and stretch, head turn, ear perk, tail wag, expression), so the whole animation set is data rather than drawn art. Playing a clip is picking a UV rect out of the atlas.

Clips: idle, look around, peek, sleep, wake, walk, run, sprint, jump rise, jump fall, land, flip, climb, sit, stretch, groom, scratch, wave, dance, salute, shrug, point, sneeze, eat, play, idle talk, hide, alert, panic, cheer, dragged, tumble, bounce, dizzy, trip.

### The state machine

States behave like a boss's moveset. Each one has a duration, a priority, an interrupt rule and a cooldown, defined in one table in `src/companion.rs`:

- **Reactions** (sleep, wake, alert, panic, cheer) are driven by the machine's load and out-rank whatever idle behaviour is playing.
- **Idle behaviours** are picked by weight once nothing is forcing the issue.
- **Cooldowns** stop the same move firing repeatedly, and weighted picks mean one behaviour cannot dominate.

There are 37 states in total, and he can be picked up, carried, thrown and bounced regardless of which one is running.

### Settings

Everything is tunable from Settings, grouped into Presentation, Window, Movement, Physics, Reactions, Timing, and a **Per-state tuning** table with a duration, weight and cooldown multiplier for each of the 37 states individually. You can also recolour him, change sprite scale and animation speed, and there are buttons to force any state and to toss him across the screen.

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

Covers the layout grid (overlap rejection, clamping, and the no-change detection that keeps settings from being rewritten on every frame of a drag), the `pmset` battery parsing that the UI depends on, and the click-through behaviour of the companion overlay.

## Project layout

- `src/main.rs` — native application entry point.
- `src/app.rs` — views, layout, theme wiring, and persistence.
- `src/metrics.rs` — background system sensor collection.
- `src/gpu.rs` — IOKit `IOAccelerator` GPU reader.
- `src/audio.rs` — Core Audio default-output worker.
- `src/autostart.rs` — per-user LaunchAgent login item.
- `src/discord.rs` — Discord Rich Presence UNIX IPC worker.
- `src/notifications.rs` and `src/sound.rs` — macOS notification and system-sound workers.
- `src/companion.rs` — sprite-driven NPC: state machine and rigid body.
- `src/sprite.rs` — procedural pixel-art sprite sheet generator and atlas.
- `src/companion_settings.rs` — every tunable value for the companion.
- `src/overlay.rs` — the always-on-top window that floats over other apps.
- `build.sh` — Intel macOS `.app` bundle build.
- `tests/integration.rs` — layout grid and battery parsing regression tests.
- `tests/hit_test.rs` — click-through regression test for the companion overlay.
