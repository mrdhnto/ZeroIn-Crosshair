# ZeroIn

A lightweight, customizable crosshair overlay for Windows, written in Rust.

Renders a Direct2D-powered transparent overlay that sits on top of all your games and applications. Configure it once and toggle it from the system tray.

## Features

- **6 crosshair types** — Dot, Cross, T-shape, Circle, Diamond, Arrow
- **Fully configurable** — size, thickness, color (hex), opacity, center dot, border, gap width, rotation
- **Global hotkey** — toggle crosshair with a configurable hotkey (default `CTRL` + `\`)
- **Per-monitor DPI aware** — crisp rendering on any display scaling
- **Click-through overlay** — mouse events pass straight to the window behind
- **System tray** — toggle on/off, switch types, reload config without restarting
- **Persistent config** — reads `config.ini` next to the executable

## Usage

1. Download the latest release or build from source.
2. Place `config.ini` next to the executable (optional — defaults apply otherwise).
3. Run `ZeroIn.exe` — it lives in the system tray.
4. Right-click the tray icon to:
   - Toggle crosshair on/off
   - Switch crosshair type
   - Reload config

## Configuration

Edit `config.ini` (placed next to the executable). Inline comments start with `;`:

```ini
[crosshair]
type = t              ; dot | cross | t | circle | diamond | arrow
size = 32             ; crosshair size in pixels
thickness = 1         ; line thickness
color = #00FFCC     ; hex color
dot_center = true     ; show center dot
opacity = 0.9         ; 0.0 to 1.0
border = false        ; outline mode (circle only)
space_width = 6       ; gap between center and crosshair arms
primary_key = CTRL    ; modifier: CTRL | SHIFT | ALT | WIN (or combined like CTRL+SHIFT)
secondary_key = \     ; key: letter, number, F-key, or symbol (\, -, =, [, ], etc.)
rotation = 0.0        ; rotation in degrees
```

Default config applies if the file is missing or a value is invalid. Invalid values are logged to `ZeroIn.log` next to the executable.

## Build from Source

**Requirements:**
- Rust edition 2024 (nightly)
- Windows (uses Win32 + Direct2D APIs)

```sh
git clone https://github.com/YOUR_USER/ZeroIn
cd ZeroIn
cargo build --release
```

The binary will be at `target/release/ZeroIn.exe`. Place `config.ini` and optionally `icon.ico` next to it.

## Technical

- Uses `windows` crate (Win32 API) for overlay window, Direct2D rendering, and tray icon
- Renders on a transparent layered window (`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST`)
- Crosshair drawn with Direct2D primitives (ellipses, rectangles, lines) via `UpdateLayeredWindow`
- Hotkey registered via `RegisterHotKey` (global system-wide) with `WM_HOTKEY` message handling
- Per-monitor DPI awareness via `SetProcessDpiAwarenessContext`

## License

MIT
