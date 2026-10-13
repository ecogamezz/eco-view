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

Click the round chip on the dashboard to let him out (right-click it to call him over to the dashboard). He lives on top of your whole desktop, not inside the app.

### How he lives on the desktop

- **A real transparent overlay.** He's drawn in a borderless, always-on-top window that covers the display, joins every Space and sits over full-screen apps. The window is fully transparent, and it passes clicks through everywhere except where he actually is. The global cursor is polled from CoreGraphics every frame, so the overlay only catches the mouse while the cursor is over him or he's being carried. After you let go, focus goes back to the app you were using.
- **He walks on your windows.** Other apps' window rectangles come from `CGWindowListCopyWindowInfo` (bounds only, so no Screen Recording permission is needed). Their title-bar edges become one-way platforms, with the parts covered by a window in front cut out. The dashboard's cards are platforms too. If you drag a window he's standing on, he rides along. If you close it, he falls.
- **He gets around.** He walks, runs, jumps between windows on a solved ballistic arc, climbs the sides of tall windows and hauls himself over the top, and steps off edges to drop down. The floor is the top of the Dock and the ceiling is the menu bar.
- **His own render loop.** The overlay is a deferred egui viewport, so he keeps moving while the dashboard is minimised. He drops to 20 fps when he's asleep and nothing is happening.

### Art and animation

He's drawn as anti-aliased vector shapes tessellated straight into an egui mesh, with no sprite sheet, so he stays sharp at any size and on Retina. Shapes in a group draw their outlines first and then their fills, which gives a clean cartoon silhouette. Fills have soft gradients.

Animation is procedural:

- A pose per activity is blended smoothly into the next.
- A skeleton is built from the pose: hips, a spine that can rear up, a head, four two-bone IK legs and a seven-segment ringed tail.
- The gait phase is driven by the distance he actually covers, so his feet don't slide.
- Secondary motion includes squash and stretch, a tail that lags behind movement, breathing, blinking, ear twitches, a smooth turn-around and eyes that follow the cursor.
- When you carry him, he hangs from the cursor like a pendulum: he swings when you move, his legs dangle and his tail sags.

There are six coats (Classic, Silver, Ginger, Midnight, Snow, Neon) plus a custom one.

### What he does

There are 50 activities, defined in one boss-style table in `src/raccoon/acts.rs` (duration range, cooldown, weight, priority, interrupt rule):

- **Idle:** idle, look around, sit, loaf, sleep, yawn, stretch, groom, wash paws, scratch, sniff, rummage, snack, wave, dance, chase his tail, roll over, sneeze, hop, backflip.
- **Getting around:** wander, run, explore, jump, climb, mantle, fall, land.
- **With you:** curious (tilts his head at the cursor), follow the cursor, chase the cursor, stalk and pounce.
- **Reactions to the Mac:** alert, then panic on a CPU or GPU spike. Overheating (fans himself with a leaf) on sustained load or high temperature. Cheering when the load drops or the charger is plugged in. Low battery. Memory full. Watching a big download.
- **Reactions to you:** petted (rub the cursor back and forth over him), booped (click), annoyed (too many clicks), carried, thrown, bounce, splat (thrown hard into a screen edge), dizzy, get up.

Needs (energy, boredom, affection) shift which activities he picks: he gets sleepy when tired or late at night and playful when bored. Speech bubbles comment on what's happening ("CPU at 97%!", "Battery at 12%... sleepy."), and particles add dust, hearts, stars, sweat, confetti, music notes, crumbs and Zzz. Optional sounds use macOS system sounds, including Purr.

### Settings

Under Settings → Raccoon companion:

- **Status:** a live readout with energy, boredom and affection meters, plus buttons to call him over, toss him, nap, wake, or force any activity.
- **Appearance:** size, animation speed, outline, coat or custom colours, shadow, name tag, effects, speech bubbles and chattiness.
- **Behaviour:** liveliness, walking on windows and cards, climbing, following and playing with the cursor, petting, decorative click-through mode, and nap rules.
- **Reactions:** thresholds for each machine reaction.
- **Movement and Physics:** every speed, gravity, bounce, throw and dangle constant.
- **Sound.**
- **Per-activity tuning:** length, chance and cooldown for each of the 50 activities.

### Developer preview

```zsh
cargo run -- --raccoon-sheet sheet.png                       # every activity on one PNG
RACCOON_SHEET_ONLY=0,14,38 cargo run -- --raccoon-sheet a.png  # just a few
```

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
