<div align="center">
  <img src="icon.png" width="120" alt="ZeroIn">
  <h1>ZeroIn</h1>
  <p><i>"A zero-overhead, anti-cheat-safe crosshair overlay for Windows and Linux gamers"</i></p>

  <p>
    <img src="https://img.shields.io/github/v/release/mrdhnto/ZeroIn-Crosshair?style=for-the-badge&logo=ghost&color=00ffff" alt="Version">
    <img src="https://img.shields.io/badge/Platform-Windows-8a2be2?style=for-the-badge&logo=windows11" alt="Windows">
    <img src="https://img.shields.io/badge/Platform-Linux-fcc624?style=for-the-badge&logo=linux" alt="Linux">
    <img src="https://img.shields.io/badge/Rust-stable-e05a2c?style=for-the-badge&logo=rust" alt="Rust">
    <img src="https://img.shields.io/badge/License-MIT-00ff7f?style=for-the-badge" alt="License">
    <img src="https://img.shields.io/github/actions/workflow/status/mrdhnto/ZeroIn-Crosshair/ci.yml?style=for-the-badge&logo=githubactions&label=CI" alt="CI">
  </p>
</div>

The lightweight, low-latency game crosshair overlay that works with any FPS title. No bloat. Just a tray icon and a config file.

Renders a transparent on-screen crosshair using Direct2D (Windows) or a software rasterizer (Linux), sitting on top of any game or application in borderless windowed mode. Configure it once and toggle it from the system tray. The ultimate custom crosshair for any game.

## Features

- **6 crosshair types** : Dot, Cross, T-shape, Circle, Diamond, Arrow
- **PNG image crosshair** : load any PNG file as crosshair (overrides geometry types)
- **Fully configurable** : size, thickness, color (hex), opacity, center dot, border, border size, gap width, rotation, center dot size
- **Independent H/V arm thickness** : separate thickness control for horizontal and vertical arms
- **Background outline for all types** : configurable color border behind any crosshair shape
- **Global hotkey** : toggle crosshair with a configurable hotkey (default `CTRL` + `\`)
- **Named profiles** : save/load named profiles from `presets.json` via tray menu
- **Use config.ini mode** : detach from profiles and fall back to raw `config.ini` settings (default state)
- **Instant auto-reload** : `config.ini` changes are detected instantly via file system events (no polling)
- **Opacity quick presets** : set opacity directly from the tray submenu
- **Per-monitor DPI aware** : crisp rendering on any display scaling
- **Multi-monitor support** : mirror crosshair across all monitors, or pick a specific monitor via config or tray menu
- **Position adjustment** : fine-tune crosshair XY offset from center (`adjust_x` / `adjust_y`)
- **Click-through overlay** : mouse events pass straight to the window behind
- **System tray** : toggle on/off, switch types, pick monitor, reload config without restarting
- **Persistent config** : reads `config.ini` next to the executable

## Why ZeroIn?

Most crosshair overlays are AutoHotkey scripts, Electron apps, or game-specific hacks. This lightweight Rust-based crosshair overlay works on both Windows and Linux and is different:

| ZeroIn | AHK scripts | Electron overlays | Game-specific tools |
|---|---|---|---|
| ~800KB binary | Needs AHK runtime | 100MB+ with bundled runtime | Only one game |
| Direct2D (Win) / SwCanvas (Linux) | GDI-based (slower) | GPU compositor overhead | Game-dependent |
| PNG crosshair support | Shapes only | Usually shapes only | Limited shapes |
| Works with any borderless windowed game | Untested/unreliable | Untested/unreliable | Only one game |
| Per-monitor DPI aware | No | Mostly no | Usually no |
| Named profiles per game | Manual switching | Manual switching | One profile |
| Instant config auto-reload (fsnotify) | No | Restart required | Restart required |
| Open source MIT | Yes | Rarely | No |

ZeroIn is a focused, minimal crosshair overlay for FPS games on Windows and Linux that does one thing well: render a custom crosshair you can see in any title without getting in the way. Low latency, transparent overlay — no process access, nothing for kernel-level anti-cheat to conventionally detect.

## Compatibility & Anti-Cheat Notes

**Important:** "Not detected by anti-cheat software" and "permitted by the game's Terms of Service" are different things. Riot, Valve, and Activision's ToS may restrict any overlay that alters in-game visual information. This is not legal advice — use at your own discretion. Anti-cheat behavior also changes with updates, so there are never zero-risk guarantees.

**Windows:** ZeroIn uses `UpdateLayeredWindow` with `WS_EX_TRANSPARENT`. The same transparent overlay technique as Discord and Steam overlays.

**Linux:** ZeroIn uses `winit` to create a transparent always-on-top window, `softbuffer` for pixel buffer display, and X11 ShapeMask for click-through. Works on X11 sessions; Wayland has limited support (tray icon works, but global hotkeys and click-through are unavailable).

**Does not interact with game processes.** ZeroIn does not inject, hook, read memory, or modify any process. It is a pure transparent window with no process access — the same overlay technique used by Discord and Steam.

| Game | Anti-Cheat | Status |
|---|---|---|
| Counter-Strike 2 | VAC | ✓ Tested in ranked match, no issues |
| Valorant | Vanguard | ✓ Tested in competitive, no issues |
| PUBG | BattlEye | ✓ Tested in ranked match, no issues |
| Apex Legends | Easy Anti-Cheat | ✓ Functional test only, no issues |
| Fortnite | BattlEye + EAC | ✓ Functional test only, no issues |
| Overwatch 2 | - | ✓ Functional test only, no issues |

Kernel-level anti-cheat (Vanguard, BattlEye, EAC, Ricochet) operates at a lower ring level than ZeroIn's user-mode overlay. ZeroIn never touches the target process — no handles, no threads, no injection, no hooks. It is a transparent click-through window, not a game modification.

CS2, Valorant, and PUBG above were personally tested in ranked/competitive play. Apex Legends, Fortnite, and Overwatch 2 were functionally tested only — if you play those in ranked queues, exercise your own judgment.

**If your game supports borderless windowed / display borderless windowed mode, ZeroIn will work.** This covers virtually every modern title.

## Usage

1. [Download the latest release](https://github.com/mrdhnto/ZeroIn-Crosshair/releases/latest).
2. Place `config.ini` next to the executable (optional, defaults apply otherwise).
3. Run `ZeroIn.exe` (Windows) or `./ZeroIn` (Linux). It lives in the system tray.
4. Right-click the tray icon to:
   - Toggle crosshair on/off
   - Switch crosshair type
   - Choose opacity preset
   - Pick monitor or mirror all monitors
   - Select a named profile, use config.ini directly, save current settings, or save as new profile
   - Reload profiles from disk

## Configuration

Edit `config.ini` (placed next to the executable). Inline comments start with `;`:

```ini
[crosshair]
type = t              ; dot | cross | t | circle | diamond | arrow
size = 32             ; crosshair size in pixels
thickness = 1         ; line thickness (fallback for thickness_h/thickness_v)
thickness_h = 1       ; horizontal arm thickness (overrides thickness)
thickness_v = 1       ; vertical arm thickness (overrides thickness)
color = #00FFCC       ; hex color
dot_center = true     ; show center dot
dot_size = 1.5        ; center dot radius (0.5 to 50)
opacity = 0.9         ; 0.0 to 1.0
border = true         ; enable background outline for all crosshair types
border_size = 1.0     ; thickness of background outline (0 to disable)
space_width = 6       ; gap between center and crosshair arms
primary_key = CTRL    ; modifier: CTRL | SHIFT | ALT | WIN (or combined like CTRL+SHIFT)
secondary_key = \     ; key: letter, number, F-key, or symbol (\, -, =, [, ], etc.)
rotation = 0.0        ; rotation in degrees
png_crosshair =       ; path to PNG file to use as crosshair (overrides type, respects size/rotation/opacity)
mirror_crosshair = false ; show crosshair on all monitors (true/false)
set_monitor = 0       ; which monitor to use (0=primary). Ignored when mirror_crosshair=true
adjust_x = 0          ; fine-tune X position from center (pixels, negative=left)
adjust_y = 0          ; fine-tune Y position from center (pixels, negative=up)
```

Default config applies if the file is missing or a value is invalid. Invalid values are logged to `ZeroIn.log` next to the executable.

## Known Limitations

- **Exclusive fullscreen** : some older titles in exclusive fullscreen may hide the overlay. Run in **borderless windowed mode** (display borderless windowed) for guaranteed compatibility, virtually all modern games support this.
- **Not captured by OBS** : the overlay is visible on screen but may not appear in OBS without game capture source.
- **Linux X11-only** : requires the X11 display server. Wayland is detected at runtime; global hotkeys and click-through are unavailable on Wayland (tray icon still works).
- **Software rendered on Linux** : crosshair is rasterized in software via SwCanvas (CPU) with per-frame redraw, resulting in higher CPU usage compared to Windows. A GPU-accelerated rendering pipeline (wgpu) is planned for a future update.

## Build from Source

**Requirements:**
- Rust edition 2024 (stable, ≥1.85)

**Windows:**
```sh
git clone https://github.com/mrdhnto/ZeroIn-Crosshair
cd ZeroIn
cargo build --release
```
The binary will be at `target/release/ZeroIn.exe`. Place `config.ini` and optionally `icon.ico` next to it.

**Linux prerequisites:**
```sh
sudo apt install build-essential pkg-config libgtk-3-dev libx11-dev \
  libxkbcommon-dev libxcb-shape0-dev libxcb-xfixes0-dev libxcb-randr0-dev \
  libxcb-composite0-dev libxcb-xkb-dev libxcb-xinput-dev \
  libxcb-xinerama0-dev libxcb-cursor-dev libasound2-dev
```
*Tray icon on Ubuntu ≥24.04: install `libayatana-appindicator-dev` instead of `libappindicator-dev`, or try `libayatana-appindicator3-dev` if not found.*

**Linux:**
```sh
git clone https://github.com/mrdhnto/ZeroIn-Crosshair
cd ZeroIn
cargo build --release
```
The binary will be at `target/release/ZeroIn`. Place `config.ini` next to it.

## Technical

**Windows:**
- Uses `windows` crate (Win32 API) for overlay window, Direct2D rendering, and tray icon
- Renders on a transparent layered window (`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST`)
- Crosshair drawn with Direct2D primitives (ellipses, rectangles, lines) via `D2DCanvas` wrapper
- PNG crosshair decoded via `image` crate, rendered as `ID2D1Bitmap` with premultiplied alpha
- Per-monitor DPI awareness via `SetProcessDpiAwarenessContext`
- Multi-monitor: one overlay window per monitor (mirror mode), or single overlay on selected monitor
- Monitor enumeration via `EnumDisplayMonitors`; hotplug handled via `WM_DISPLAYCHANGE` with overlay reconciliation
- Position adjustment via `adjust_x`/`adjust_y` offset from each overlay's center
- Hotkey via `RegisterHotKey` (global system-wide) with `WM_HOTKEY` message handling
- Click-through via `WS_EX_TRANSPARENT` extended window style
- Opacity via `UpdateLayeredWindow` with `BLENDFUNCTION.SourceConstantAlpha`

**Linux:**
- Uses `winit` + `softbuffer` for overlay window and pixel buffer display
- Crosshair rendered via `SwCanvas` software rasterizer (premultiplied BGRA, bilinear rotation)
- Global hotkey via `x11rb::GrabKey` background thread with comprehensive VK→X11 keysym mapping, sends toggle via `EventLoopProxy`
- Click-through via `set_cursor_hittest(false)` (winit) + X11 `shape_rectangles` for input passthrough
- System tray via `tray-icon` crate in a dedicated GTK thread (`gtk::main_iteration_do` pump), with radio-opacity and toggle-checked commands bridged via channel
- GTK 3.24+ required for tray icon (libappindicator / ayatana-appindicator)
- Instant config auto-reload via `notify` crate — file system events trigger immediate reload (no polling)
- Multi-monitor: one overlay window per monitor with per-monitor `SwCanvas`, reconciliation via `determine_targets`/`reconcile_overlays`
- Monitor enumeration via `winit::Window::available_monitors` on each `AboutToWait` refresh
- Tray icon loaded from embedded `icon.png` at compile time, with file fallback

**Cross-platform:**
- Profiles serialized as `presets.json` via `serde_json` — select a named profile from the tray menu, or choose **Use config.ini** to detach and read directly from `config.ini`
- Profile reload from disk re-applies the active profile to the visual state immediately
- PNG crosshair decoding via `image` crate
- `Canvas` trait abstracts drawing primitives — `crosshair.rs` is pure math, no platform deps

## Preview

![ZeroIn crosshair over CS2](screenshots/crosshair-cs2.png)
![ZeroIn crosshair over Valorant](screenshots/crosshair-valorant.png)

## Project Status

ZeroIn is in active development. Feature depth (6 crosshair types, PNG support, independent H/V thickness, named profiles, instant fsnotify reload, multi-monitor, per-monitor DPI) already exceeds most alternatives — including linux support. Expect steady improvements until feature-complete, then maintenance-only.

If you have ideas, want to contribute features (help is very welcome), or just want to report a bug — I highly appreciate it.

**Current focus:**
- Tests and CI benchmarks
- Dependency auditing (deny.toml)

## Contributing

PRs and issues welcome. Check the [open issues](https://github.com/mrdhnto/ZeroIn-Crosshair/issues) for planned work or suggest your own.

## License

This project is licensed under the [MIT License](LICENSE).
