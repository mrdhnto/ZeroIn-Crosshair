# ZeroIn

![Windows](https://img.shields.io/badge/platform-Windows-blue)
![Rust](https://img.shields.io/badge/rust-nightly-orange)
![License](https://img.shields.io/badge/license-MIT-green)
[![GitHub Release](https://img.shields.io/github/v/release/mrdhnto/ZeroIn)](https://github.com/mrdhnto/ZeroIn/releases/latest)

A zero-overhead, anti-cheat-safe crosshair overlay for Windows gamers. No bloat. Just a tray icon and a config file.

Renders a Direct2D-powered transparent overlay that sits on top of any game or application. Configure it once and toggle it from the system tray.

## Features

- **6 crosshair types**: Dot, Cross, T-shape, Circle, Diamond, Arrow
- **PNG image crosshair** : load any PNG file as crosshair (overrides geometry types)
- **Fully configurable**: size, thickness, color (hex), opacity, center dot, border, gap width, rotation, center dot size
- **Global hotkey** : toggle crosshair with a configurable hotkey (default `CTRL` + `\`)
- **Named profiles**: save/load named profiles from `presets.json` via tray menu
- **Auto-reload**: `config.ini` changes are detected and reloaded automatically
- **Opacity quick presets**: set opacity directly from the tray submenu
- **Per-monitor DPI aware**: crisp rendering on any display scaling
- **Click-through overlay**: mouse events pass straight to the window behind
- **System tray**: toggle on/off, switch types, reload config without restarting
- **Persistent config**: reads `config.ini` next to the executable

## Why ZeroIn?

Most crosshair overlays are AutoHotkey scripts, Electron apps, or game-specific hacks. ZeroIn is different:

| ZeroIn | AHK scripts | Electron overlays | Game-specific tools |
|---|---|---|---|
| ~512KB binary | Needs AHK runtime | 100MB+ with bundled runtime | Only one game |
| Direct2D hardware acceleration | GDI-based (slower) | GPU compositor overhead | Game-dependent |
| PNG crosshair support | Shapes only | Usually shapes only | Limited shapes |
| Works with Vanguard (Valorant) | Untested/unreliable | Untested/unreliable | N/A |
| Per-monitor DPI aware | No | Mostly no | Usually no |
| Named profiles per game | Manual switching | Manual switching | One profile |
| Config auto-reload | No | Restart required | Restart required |
| Open source MIT | Yes | Rarely | No |

It is a focused, minimal overlay that does one thing well: render a crosshair you can see in any game without getting in the way.

## Compatibility & Anti-Cheat

ZeroIn uses `UpdateLayeredWindow` with `WS_EX_TRANSPARENT`. The same transparent overlay technique as Discord and Steam overlays.

| Game | Anti-Cheat | Status |
|---|---|---|
| Counter-Strike 2 | VAC | ✓ Tested — no issues |
| Valorant | Vanguard | ✓ Tested — no issues |
| Apex Legends | Easy Anti-Cheat | ✓ Expected — same technique as Discord overlay |
| Fortnite | BattlEye + EAC | ✓ Expected — no injection or hooking |

Kernel-level anti-cheat (Vanguard, BattlEye) operates at a lower level than ZeroIn's user-mode overlay. ZeroIn does not inject, hook, or modify any process, it is a transparent click-through window, not a game modification.

## Usage

1. [Download the latest release](https://github.com/mrdhnto/ZeroIn/releases/latest).
2. Place `config.ini` next to the executable (optional — defaults apply otherwise).
3. Run `ZeroIn.exe` — it lives in the system tray.
4. Right-click the tray icon to:
   - Toggle crosshair on/off
   - Switch crosshair type
   - Choose opacity preset
   - Select a named profile, save current settings, or save as new profile
   - Reload config

## Configuration

Edit `config.ini` (placed next to the executable). Inline comments start with `;`:

```ini
[crosshair]
type = t              ; dot | cross | t | circle | diamond | arrow
size = 32             ; crosshair size in pixels
thickness = 1         ; line thickness
color = #00FFCC       ; hex color
dot_center = true     ; show center dot
dot_size = 1.5        ; center dot radius (0.5 to 50)
opacity = 0.9         ; 0.0 to 1.0
border = false        ; outline mode (circle only)
space_width = 6       ; gap between center and crosshair arms
primary_key = CTRL    ; modifier: CTRL | SHIFT | ALT | WIN (or combined like CTRL+SHIFT)
secondary_key = \     ; key: letter, number, F-key, or symbol (\, -, =, [, ], etc.)
rotation = 0.0        ; rotation in degrees
png_crosshair =       ; path to PNG file to use as crosshair (overrides type, respects size/rotation/opacity)
```

Default config applies if the file is missing or a value is invalid. Invalid values are logged to `ZeroIn.log` next to the executable.

## Known Limitations

- **Windows only** — requires Win32 + Direct2D APIs. Linux support is being explored via a GPU abstraction layer.
- **Single monitor** — the overlay renders on your primary display. Multi-monitor spanning is not yet supported.
- **Polling config reload** — changes are detected every 2 seconds (not instant file system watching).
- **Exclusive fullscreen** — some older titles in exclusive fullscreen may hide the overlay. Run in borderless windowed mode if this happens.
- **Not captured by OBS** : the overlay uses `WS_EX_TRANSPARENT` for click-through. It is visible on screen but may not appear in OBS without game capture source.

## Build from Source

**Requirements:**
- Rust edition 2024 (nightly)
- Windows (uses Win32 + Direct2D APIs)

```sh
git clone https://github.com/mrdhnto/ZeroIn
cd ZeroIn
cargo build --release
```

The binary will be at `target/release/ZeroIn.exe`. Place `config.ini` and optionally `icon.ico` next to it.

## Roadmap

- **v0.3**: Save/load named profiles, opacity quick presets, adjustable center dot size, PNG crosshair support
- **v0.4.1**: Border outlines for all crosshair types, independent horizontal/vertical arm thickness
- **v0.4.2**: Breathe animation, crosshair animation via IPC
- **Future**: Linux support via GPU abstraction layer

<!-- All planned features are tracked as [GitHub Issues](https://github.com/mrdhnto/ZeroIn/issues). -->

## Technical

- Uses `windows` crate (Win32 API) for overlay window, Direct2D rendering, and tray icon
- PNG crosshair decoding via `image` crate, drawn as `ID2D1Bitmap` with premultiplied alpha
- Profiles serialized as `presets.json` via `serde_json`
- Renders on a transparent layered window (`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST`)
- Crosshair drawn with Direct2D primitives (ellipses, rectangles, lines) or bitmaps via `UpdateLayeredWindow`
- Hotkey registered via `RegisterHotKey` (global system-wide) with `WM_HOTKEY` message handling
- Per-monitor DPI awareness via `SetProcessDpiAwarenessContext`
- Config auto-reload via `SetTimer` polling (`WM_TIMER` every 2s)

## Contributing

PRs and issues welcome. Check the [open issues](https://github.com/mrdhnto/ZeroIn/issues) for planned work or suggest your own.

## License

MIT
