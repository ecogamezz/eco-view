# eco view

**eco view** is a macOS desktop system monitor for Intel Macs. It is built in Rust with eframe/egui and the OpenGL (`glow`) renderer, which remains the reliable GPU path on x86_64 MacBooks.

## Interface

- **Sidebar navigation** with Monitor (Overview, Performance, Processes) and Hardware (Storage, Devices, System) groups, live badges, a system-health widget, and a collapsed icon-only mode (⌘B). Content runs under a transparent title bar; drag the window by the sidebar top or the page header.
- **Overview:** a health score (0–100) with a one-line verdict and ring gauges for CPU, memory, GPU and disk, above a dashboard of twelve cards on a 48-column grid. Choose **Edit layout** (⌘E) to drag cards by their grip or resize them from the corner; cards can't overlap. Click a card's header to open its detail page.
- **Performance:** large charts for processor (with per-core bars), memory, graphics, network and temperature, each with now / average / peak. The window can be 1, 5 or 15 minutes. Hover any chart for a crosshair and exact values.
- **Processes:** search by name or PID and sort by CPU, memory, name or PID. Rows are virtualised and show inline usage bars. There's a details panel, plus a right-click menu to copy the name or PID or to **Quit** / **Force quit**, both behind a confirmation dialog.
- **Storage:** total capacity across volumes, then a card per volume with a capacity ring, file system and mount point.
- **Devices:** USB, Bluetooth, input devices and game controllers as searchable, filterable tiles.
- **System:** an "About this Mac" view (model, chip, cores, memory, GPU model, macOS, kernel, power, sensors) with **Copy specs**.
- **Command palette (⌘K):** jump to any page, switch theme, toggle the raccoon, edit or reset the layout, copy specs, or open a settings section.
- **Alerts:** threshold alerts show as a banner, a toast stack and an optional macOS notification. An alert centre behind the bell keeps the history with timestamps, and a quiet period stops repeats.
- **Settings:** sectioned like System Settings (General, Appearance, Dashboard, Alerts, Sound, Discord, Raccoon, About), with toggles, segmented controls and live values next to each threshold.
- **Themes:** six presets (Midnight, Graphite, Aurora, Ember, Daylight, Paper) with live preview tiles, an optional custom accent, text size, compact spacing, reduced motion, and an optional ambient background effect.
- **Typography:** Avenir Next and SF Mono from macOS. Everything is vector-drawn (around 40 line icons, rings, charts), so it stays sharp on Retina.

Keyboard: ⌘K palette · ⌘1–⌘6 pages · ⌘, settings · ⌘E edit layout · ⌘B sidebar · Esc closes popovers.

## Monitoring

- Live CPU load, per-core activity and load averages; memory and swap; GPU load and VRAM when IOKit `IOAccelerator` publishes them, with the GPU's model name taken from its PCI device; network throughput and session totals; storage; battery and time remaining; uptime and boot time; and processes.
- Fifteen minutes of history per series at the default one-second refresh.
- Native Core Audio output volume and mute for the default output device. eco view never records audio.
- SMC thermal readings through `sysinfo` components when the firmware exposes them (CPU proximity `TC0P`, PECI `TXCX`, GPU `TG0P`, battery `TB0T` on Intel).
- USB, Bluetooth, HID and game-controller inventory from IOKit, on a slower background cadence.
- Optional Discord Rich Presence over Discord's local IPC socket, with placeholders such as `{cpu}`, `{ram}`, `{gpu}`, `{temp}`, `{host}`, `{uptime}`, `{network_down}` and `{network_up}`.
- Optional Login Item via a per-user LaunchAgent, a custom Dock icon (copied into Application Support), and system-sound cues.
- Settings are JSON in `~/Library/Application Support/eco-view/settings.json` and save automatically. Older files keep loading; a dashboard layout saved by an older version is reset once to the new grid.
- Sampling runs on background threads. The window repaints only when something changes or animates.

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
cargo run -- --screenshot overview shot.png                   # render any page headlessly
cargo run -- --screenshot settings/appearance a.png --theme daylight --pointer 900,380
cargo run -- --screenshot overview p.png --palette            # with the command palette open
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
