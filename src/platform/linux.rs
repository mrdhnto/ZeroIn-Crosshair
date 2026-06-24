use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use winit::dpi::PhysicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::window::{CursorIcon, WindowLevel, Window};

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::TrayIconBuilder;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as XProtoConnectionExt;

use crate::canvas::{Canvas, SwCanvas, blend_premul};
use crate::config::{Config, CrosshairType, Hotkey};
use crate::crosshair;

const OPACITY_PRESETS: [f32; 6] = [0.1, 0.25, 0.5, 0.75, 0.9, 1.0];

struct MonitorInfo {
    index: u32,
    name: String,
    position: winit::dpi::PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

struct OverlayWindow {
    window: Rc<Window>,
    surface: softbuffer::Surface<OwnedDisplayHandle, Rc<Window>>,
    sw_canvas: SwCanvas,
    png_pixels: Option<(Vec<u8>, u32, u32)>,
    window_size: PhysicalSize<u32>,
    monitor_index: u32,
    monitor_name: String,
}

struct App {
    config: Config,
    crosshair_type: CrosshairType,
    visible: bool,
    config_mtime: Option<SystemTime>,
    profiles: crate::profiles::Profiles,
    overlays: Vec<OverlayWindow>,
}

enum UserEvent {
    #[allow(dead_code)]
    HotkeyPressed,
}

fn exe_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn config_mtime() -> Option<SystemTime> {
    let config_path = exe_dir().join("config.ini");
    std::fs::metadata(config_path).ok().and_then(|m| m.modified().ok())
}

fn profile_timestamp_name() -> String {
    let secs = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("Profile_{secs}")
}

fn closest_opacity_idx(opacity: f32) -> usize {
    OPACITY_PRESETS.iter()
        .enumerate()
        .min_by(|&(_, a), &(_, b)| {
            (a - opacity).abs().partial_cmp(&(b - opacity).abs()).unwrap()
        })
        .map(|(i, _)| i)
        .unwrap_or(5)
}

fn load_png_pixels(config: &Config) -> Option<(Vec<u8>, u32, u32)> {
    let path = match &config.png_crosshair {
        Some(p) => p,
        None => return None,
    };
    let path = std::path::Path::new(path);
    if !path.exists() {
        return None;
    }
    let img = image::ImageReader::open(path).ok()?.decode().ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let pixels: Vec<u8> = rgba.chunks(4).flat_map(|p| {
        let r = p[0] as u32;
        let g = p[1] as u32;
        let b = p[2] as u32;
        let a = p[3] as u32;
        vec![
            (b * a / 255) as u8,
            (g * a / 255) as u8,
            (r * a / 255) as u8,
            a as u8,
        ]
    }).collect();
    Some((pixels, w, h))
}

fn try_load_tray_icon() -> Option<tray_icon::Icon> {
    let embedded = include_bytes!("../../icon.png");
    if let Ok(img) = image::load_from_memory(embedded) {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        if w > 0 && h > 0 {
            if let Ok(icon) = tray_icon::Icon::from_rgba(rgba.into_raw(), w, h) {
                return Some(icon);
            }
        }
    }
    let paths = ["focus.png", "icon.png", "new-icon.png"];
    for name in &paths {
        let path = exe_dir().join(name);
        if let Ok(img) = image::ImageReader::open(&path) {
            if let Ok(decoded) = img.decode() {
                let rgba = decoded.to_rgba8();
                let (w, h) = rgba.dimensions();
                if let Ok(icon) = tray_icon::Icon::from_rgba(rgba.into_raw(), w, h) {
                    return Some(icon);
                }
            }
        }
    }
    None
}

struct TrayMenuIds {
    toggle_id: MenuId,
    type_ids: Vec<MenuId>,
    opacity_ids: Vec<MenuId>,
    save_current_id: MenuId,
    save_new_id: MenuId,
    reload_profiles_id: MenuId,
    exit_id: MenuId,
    mirror_id: MenuId,
    monitor_ids: Vec<MenuId>,
}

enum TrayCommand {
    SetToggleChecked(bool),
    OpacityRadio(usize),
    SetMirrorChecked(bool),
    SetMonitorChecked(Option<usize>),
}

fn load_tray_ids_and_spawn_gtk(
    profile_names: Vec<String>,
    monitor_names: Vec<(u32, String)>,
    icon: Option<tray_icon::Icon>,
) -> (TrayMenuIds, std::sync::mpsc::Sender<TrayCommand>) {
    let (id_tx, id_rx) = std::sync::mpsc::channel();
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();

    std::thread::Builder::new()
        .name("gtk-main".to_string())
        .spawn(move || {
            gtk::init().expect("GTK initialization failed");

            let toggle = CheckMenuItem::new("Toggle On/Off", true, false, None);
            let type_items: Vec<MenuItem> = ["Dot", "Cross", "T", "Circle", "Diamond", "Arrow"]
                .iter().map(|&n| MenuItem::new(n, true, None)).collect();
            let opacity_items: Vec<CheckMenuItem> = OPACITY_PRESETS.iter().map(|&val| {
                CheckMenuItem::new(&format!("{:.2}", val), true, false, None)
            }).collect();
            let save_current = MenuItem::new("Save Current", true, None);
            let save_new = MenuItem::new("Save As New Profile", true, None);
            let reload_profiles = MenuItem::new("Reload Profiles", true, None);
            let exit_item = MenuItem::new("Exit", true, None);

            // Monitor submenu
            let mirror_toggle = CheckMenuItem::new("Mirror All Monitors", true, false, None);
            let mut monitor_items: Vec<CheckMenuItem> = Vec::new();
            for &(ref _idx, ref name) in &monitor_names {
                let item = CheckMenuItem::new(name, true, false, None);
                monitor_items.push(item);
            }

            let ids = TrayMenuIds {
                toggle_id: toggle.id().clone(),
                type_ids: type_items.iter().map(|i| i.id().clone()).collect(),
                opacity_ids: opacity_items.iter().map(|i| i.id().clone()).collect(),
                save_current_id: save_current.id().clone(),
                save_new_id: save_new.id().clone(),
                reload_profiles_id: reload_profiles.id().clone(),
                exit_id: exit_item.id().clone(),
                mirror_id: mirror_toggle.id().clone(),
                monitor_ids: monitor_items.iter().map(|i| i.id().clone()).collect(),
            };

            let menu = Menu::new();
            menu.append(&toggle).unwrap();
            menu.append(&PredefinedMenuItem::separator()).unwrap();

            let type_submenu = Submenu::new("Crosshair Type", true);
            for item in &type_items {
                type_submenu.append(item).unwrap();
            }
            menu.append(&type_submenu).unwrap();
            menu.append(&PredefinedMenuItem::separator()).unwrap();

            let opacity_submenu = Submenu::new("Opacity", true);
            for item in &opacity_items {
                opacity_submenu.append(item).unwrap();
            }
            menu.append(&opacity_submenu).unwrap();
            menu.append(&PredefinedMenuItem::separator()).unwrap();

            // Monitor submenu
            let monitor_submenu = Submenu::new("Monitor", true);
            monitor_submenu.append(&mirror_toggle).unwrap();
            monitor_submenu.append(&PredefinedMenuItem::separator()).unwrap();
            for item in &monitor_items {
                monitor_submenu.append(item).unwrap();
            }
            menu.append(&monitor_submenu).unwrap();
            menu.append(&PredefinedMenuItem::separator()).unwrap();

            let profiles_submenu = Submenu::new("Profiles", true);
            for (i, name) in profile_names.iter().enumerate() {
                let item = MenuItem::new(name, true, None);
                profiles_submenu.insert(&item, i).unwrap();
            }
            profiles_submenu.append(&save_current).unwrap();
            profiles_submenu.append(&save_new).unwrap();
            profiles_submenu.append(&reload_profiles).unwrap();
            menu.append(&profiles_submenu).unwrap();
            menu.append(&PredefinedMenuItem::separator()).unwrap();

            menu.append(&exit_item).unwrap();

            let mut builder = TrayIconBuilder::new().with_tooltip("ZeroIn Crosshair");
            if let Some(ic) = icon {
                builder = builder.with_icon(ic);
            }
            let _tray_icon = builder.with_menu(Box::new(menu)).build();

            id_tx.send(ids).ok();

            loop {
                gtk::main_iteration_do(false);
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        TrayCommand::SetToggleChecked(v) => toggle.set_checked(v),
                        TrayCommand::OpacityRadio(idx) => {
                            for (i, item) in opacity_items.iter().enumerate() {
                                item.set_checked(i == idx);
                            }
                        }
                        TrayCommand::SetMirrorChecked(v) => mirror_toggle.set_checked(v),
                        TrayCommand::SetMonitorChecked(opt) => {
                            for (i, item) in monitor_items.iter().enumerate() {
                                item.set_checked(Some(i) == opt);
                            }
                        }
                    }
                }
                std::thread::yield_now();
            }
        })
        .unwrap();

    let ids = id_rx.recv_timeout(Duration::from_secs(5)).expect("GTK startup timed out");
    (ids, cmd_tx)
}

fn vk_to_keysym(vk: u16) -> Option<u32> {
    Some(match vk {
        0x41..=0x5A => vk as u32,
        0x30..=0x39 => vk as u32,
        0x20 => 0x20,
        0x0D => 0xFF0D,
        0x09 => 0xFF09,
        0x1B => 0xFF1B,
        0x08 => 0xFF08,
        0x2E => 0xFFFF,
        0x2D => 0xFF63,
        0x24 => 0xFF50,
        0x23 => 0xFF57,
        0x21 => 0xFF55,
        0x22 => 0xFF56,
        0x25 => 0xFF51,
        0x27 => 0xFF53,
        0x26 => 0xFF52,
        0x28 => 0xFF54,
        0x70..=0x87 => 0xFFBE + (vk - 0x70) as u32,
        0xBA => 0x3B,
        0xBB => 0x3D,
        0xBC => 0x2C,
        0xBD => 0x2D,
        0xBE => 0x2E,
        0xBF => 0x2F,
        0xC0 => 0x60,
        0xDB => 0x5B,
        0xDC => 0x5C,
        0xDD => 0x5D,
        0xDE => 0x27,
        0xDF => 0x5F,
        _ => return None,
    })
}

fn setup_x11_hotkey(
    config: &Config,
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
) -> Option<std::thread::JoinHandle<()>> {
    let hk = Hotkey::from_parts(&config.primary_key, &config.secondary_key)?;

    let handle = std::thread::spawn(move || {
        let (conn, screen_num) = match x11rb::connect(None) {
            Ok(c) => c,
            Err(_) => return,
        };
        let screen = &conn.setup().roots[screen_num];

        let keysym = match vk_to_keysym(hk.vk) {
            Some(ks) => ks,
            None => return,
        };

        let keycode = {
            let min_kc = conn.setup().min_keycode;
            let max_kc = conn.setup().max_keycode;
            let keycodes = match conn.get_keyboard_mapping(min_kc, max_kc - min_kc + 1) {
                Ok(c) => c,
                Err(_) => return,
            };
            let reply = match keycodes.reply() {
                Ok(r) => r,
                Err(_) => return,
            };
            let mut found = None;
            for (kc, keysyms) in reply.keysyms.chunks(2).enumerate() {
                if keysyms.contains(&keysym) || keysyms.contains(&(keysym | 0x8000)) {
                    found = Some(min_kc + kc as u8);
                    break;
                }
            }
            match found {
                Some(kc) => kc,
                None => return,
            }
        };

        let mut x11_mod: u32 = 0;
        let m = hk.modifiers;
        if m & 0x0001 != 0 { x11_mod |= 8; }
        if m & 0x0002 != 0 { x11_mod |= 4; }
        if m & 0x0004 != 0 { x11_mod |= 1; }
        if m & 0x0008 != 0 { x11_mod |= 64; }

        if conn.grab_key(
            false,
            screen.root,
            (x11_mod as u16).into(),
            keycode,
            x11rb::protocol::xproto::GrabMode::ASYNC,
            x11rb::protocol::xproto::GrabMode::ASYNC,
        ).is_err() { return; }
        let _ = conn.flush();

        loop {
            if let Ok(event) = conn.wait_for_event() {
                if event.response_type() == x11rb::protocol::xproto::KEY_PRESS_EVENT {
                    let _ = proxy.send_event(UserEvent::HotkeyPressed);
                    let _ = conn.grab_key(
                        false, screen.root, (x11_mod as u16).into(), keycode,
                        x11rb::protocol::xproto::GrabMode::ASYNC,
                        x11rb::protocol::xproto::GrabMode::ASYNC,
                    );
                    let _ = conn.flush();
                }
            }
        }
    });

    Some(handle)
}

fn render_overlay(ov: &mut OverlayWindow, cfg: &Config, ctype: CrosshairType) {
    ov.sw_canvas.clear();
    let (w, h) = (ov.sw_canvas.width(), ov.sw_canvas.height());
    if w == 0 || h == 0 { return; }

    let (rr, gg, bb) = cfg.parse_color();
    let alpha = cfg.opacity;
    let main_color = (rr, gg, bb, alpha);
    let border_color = (0.0, 0.0, 0.0, 0.5 * alpha);
    let cx = w as f32 / 2.0 + cfg.adjust_x;
    let cy = h as f32 / 2.0 + cfg.adjust_y;
    let scale = 1.0;

    let using_png = cfg.png_crosshair.as_ref().map_or(false, |p| !p.is_empty());
    if using_png {
        if ov.png_pixels.is_none() {
            ov.png_pixels = load_png_pixels(cfg);
        }
        if let Some((ref pixels, img_w, img_h)) = ov.png_pixels.clone() {
            let target_size = cfg.size * scale;
            let aspect = img_h as f32 / img_w as f32;
            let draw_w = target_size;
            let draw_h = target_size * aspect;
            let rotation = cfg.rotation.to_radians();

            if rotation != 0.0 {
                ov.sw_canvas.begin_rotation(rotation, cx, cy);
            }

            let left = cx - draw_w / 2.0;
            let top = cy - draw_h / 2.0;
            blit_png_to_swcanvas(&mut ov.sw_canvas, pixels, img_w, img_h,
                                  left, top, draw_w, draw_h, cfg.opacity);

            if rotation != 0.0 {
                ov.sw_canvas.end_rotation();
            }
        }
    } else {
        crosshair::draw(
            &mut ov.sw_canvas, main_color, Some(border_color), ctype, cx, cy,
            cfg.size * scale,
            cfg.thickness_h * scale,
            cfg.thickness_v * scale,
            cfg.dot_center,
            cfg.border,
            cfg.border_size * scale,
            cfg.space_width * scale,
            cfg.rotation,
            cfg.dot_size,
        );
    }

    let (bw, bh) = (ov.sw_canvas.width(), ov.sw_canvas.height());
    if let (Some(nw), Some(nh)) = (NonZeroU32::new(bw), NonZeroU32::new(bh)) {
        let _ = ov.surface.resize(nw, nh);
    }
    if let Ok(mut buffer) = ov.surface.buffer_mut() {
        let src = ov.sw_canvas.pixels();
        let count = buffer.len().min(src.len());
        buffer[..count].copy_from_slice(&src[..count]);
        if buffer.len() > src.len() {
            buffer[src.len()..].fill(0);
        }
        let _ = buffer.present();
    }
}

fn blit_png_to_swcanvas(sw: &mut SwCanvas, pixels: &[u8], img_w: u32, img_h: u32,
                         dest_x: f32, dest_y: f32, dest_w: f32, dest_h: f32,
                         opacity: f32) {
    let cw = sw.width() as f32;
    let ch = sw.height() as f32;
    let bx0 = dest_x.max(0.0) as u32;
    let by0 = dest_y.max(0.0) as u32;
    let bx1 = (dest_x + dest_w).min(cw).max(0.0) as u32;
    let by1 = (dest_y + dest_h).min(ch).max(0.0) as u32;
    if bx0 >= bx1 || by0 >= by1 { return; }

    let scale_x = img_w as f32 / dest_w;
    let scale_y = img_h as f32 / dest_h;
    let w = sw.width();
    let buf = sw.pixels_mut();

    for dy in by0..by1 {
        for dx in bx0..bx1 {
            let sx = (dx as f32 - dest_x) * scale_x;
            let sy = (dy as f32 - dest_y) * scale_y;
            let sx0 = sx.floor() as u32;
            let sy0 = sy.floor() as u32;
            let sx1 = (sx0 + 1).min(img_w - 1);
            let sy1 = (sy0 + 1).min(img_h - 1);
            let fx = ((sx - sx.floor()) * 255.0) as u32;
            let fy = ((sy - sy.floor()) * 255.0) as u32;
            let fx_inv = 255 - fx;
            let fy_inv = 255 - fy;

            let p00 = pixel_at(pixels, img_w, sx0, sy0);
            let p10 = pixel_at(pixels, img_w, sx1, sy0);
            let p01 = pixel_at(pixels, img_w, sx0, sy1);
            let p11 = pixel_at(pixels, img_w, sx1, sy1);

            let row0 = lerp_pixel(p00, p10, fx, fx_inv);
            let row1 = lerp_pixel(p01, p11, fx, fx_inv);
            let sampled = lerp_pixel(row0, row1, fy, fy_inv);

            let a = ((sampled >> 24) as f32 * opacity) as u32;
            let r = (((sampled >> 16) & 0xFF) as f32 * opacity) as u32;
            let g = (((sampled >> 8) & 0xFF) as f32 * opacity) as u32;
            let b = ((sampled & 0xFF) as f32 * opacity) as u32;
            let src_pixel = (a.min(255) << 24) | (r.min(255) << 16) | (g.min(255) << 8) | b.min(255);

            let idx = (dy * w + dx) as usize;
            buf[idx] = blend_premul(buf[idx], src_pixel);
        }
    }
}

fn pixel_at(pixels: &[u8], w: u32, x: u32, y: u32) -> u32 {
    let i = ((y * w + x) * 4) as usize;
    let b = pixels[i] as u32;
    let g = pixels[i + 1] as u32;
    let r = pixels[i + 2] as u32;
    let a = pixels[i + 3] as u32;
    (a << 24) | (r << 16) | (g << 8) | b
}

fn lerp_pixel(a: u32, b: u32, t: u32, inv: u32) -> u32 {
    let ba = (a >> 24) & 0xFF; let bb = (b >> 24) & 0xFF;
    let ar = (a >> 16) & 0xFF; let br = (b >> 16) & 0xFF;
    let ag = (a >> 8) & 0xFF; let bg = (b >> 8) & 0xFF;
    let ab = a & 0xFF; let bbb = b & 0xFF;
    let oa = (ba * inv + bb * t) / 255;
    let or_ = (ar * inv + br * t) / 255;
    let og = (ag * inv + bg * t) / 255;
    let ob = (ab * inv + bbb * t) / 255;
    (oa << 24) | (or_ << 16) | (og << 8) | ob
}

fn enumerate_monitors(window: &Window) -> Vec<MonitorInfo> {
    let primary = window.primary_monitor();
    window.available_monitors()
        .enumerate()
        .map(|(i, m)| {
            let is_primary = primary.as_ref().map_or(false, |p| p == &m);
            MonitorInfo {
                index: i as u32,
                name: if is_primary {
                    format!("Monitor {i} (Primary)")
                } else {
                    format!("Monitor {i}")
                },
                position: m.position(),
                size: m.size(),
            }
        })
        .collect()
}

fn determine_targets<'a>(config: &Config, monitors: &'a [MonitorInfo]) -> Vec<&'a MonitorInfo> {
    if monitors.is_empty() {
        return Vec::new();
    }
    if config.mirror_crosshair {
        monitors.iter().collect()
    } else {
        let idx = (config.set_monitor as usize).min(monitors.len().saturating_sub(1));
        vec![&monitors[idx]]
    }
}

fn create_overlays(
    el: &EventLoop<UserEvent>,
    monitors: &[MonitorInfo],
    config: &Config,
) -> Option<(Vec<OverlayWindow>, softbuffer::Context<OwnedDisplayHandle>)> {
    let targets = determine_targets(config, monitors);
    if targets.is_empty() {
        return None;
    }

    let mut overlays: Vec<OverlayWindow> = Vec::new();
    let mut context: Option<softbuffer::Context<OwnedDisplayHandle>> = None;

    for (_idx, mon) in targets.iter().enumerate() {
        let raw = el.create_window(
            Window::default_attributes()
                .with_title("ZeroIn")
                .with_visible(false)
                .with_transparent(true)
                .with_decorations(false)
                .with_active(false)
        ).ok()?;

        raw.set_outer_position(mon.position);
        let _ = raw.request_inner_size(mon.size);
        raw.set_cursor_hittest(false).ok()?;
        raw.set_cursor(CursorIcon::Crosshair);

        let window = Rc::new(raw);

        if context.is_none() {
            let display = el.owned_display_handle();
            context = Some(softbuffer::Context::new(display).ok()?);
        }
        let mut surface = softbuffer::Surface::new(context.as_ref().unwrap(), Rc::clone(&window)).ok()?;
        if let (Some(w), Some(h)) = (NonZeroU32::new(mon.size.width), NonZeroU32::new(mon.size.height)) {
            let _ = surface.resize(w, h);
        }

        let sw_canvas = SwCanvas::new(mon.size.width, mon.size.height);

        overlays.push(OverlayWindow {
            window,
            surface,
            sw_canvas,
            png_pixels: None,
            window_size: mon.size,
            monitor_index: mon.index,
            monitor_name: mon.name.clone(),
        });
    }

    Some((overlays, context.unwrap()))
}

pub fn run() {
    let event_loop = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("ZeroIn: failed to create event loop: {e}");
            return;
        }
    };
    let proxy = event_loop.create_proxy();

    let config = Config::load();

    // Create a temporary window to enumerate monitors
    let temp_window = match event_loop.create_window(
        Window::default_attributes()
            .with_visible(false)
            .with_transparent(true)
            .with_decorations(false)
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("ZeroIn: failed to create temp window: {e}");
            return;
        }
    };

    let monitors = enumerate_monitors(&temp_window);
    // Drop temp window — it was only for monitor enumeration
    drop(temp_window);

    if monitors.is_empty() {
        eprintln!("ZeroIn: no monitors found");
        return;
    }

    let (overlays, _context) = match create_overlays(&event_loop, &monitors, &config) {
        Some(v) => v,
        None => {
            eprintln!("ZeroIn: failed to create overlay windows");
            return;
        }
    };

    let app = Rc::new(RefCell::new(App {
        config: config.clone(),
        crosshair_type: config.crosshair_type,
        visible: true,
        config_mtime: config_mtime(),
        profiles: crate::profiles::Profiles::load(),
        overlays,
    }));

    let profile_names: Vec<String> = app.borrow().profiles.list.iter().map(|p| p.name.clone()).collect();
    let monitor_names: Vec<(u32, String)> = monitors.iter().map(|m| (m.index, m.name.clone())).collect();
    let tray_icon_bytes = try_load_tray_icon();

    let (tray_menu_ids, cmd_tx) = load_tray_ids_and_spawn_gtk(profile_names, monitor_names, tray_icon_bytes);

    // Show all overlays
    {
        let app_borrow = app.borrow();
        for ov in &app_borrow.overlays {
            ov.window.set_visible(true);
            ov.window.set_window_level(WindowLevel::AlwaysOnTop);
        }
    }

    // Initial tray state
    if config.mirror_crosshair {
        cmd_tx.send(TrayCommand::SetMirrorChecked(true)).ok();
    } else {
        cmd_tx.send(TrayCommand::SetMonitorChecked(Some(config.set_monitor as usize))).ok();
    }
    cmd_tx.send(TrayCommand::SetToggleChecked(true)).ok();

    // Request redraw for all overlays initially
    {
        let app_borrow = app.borrow();
        for ov in &app_borrow.overlays {
            ov.window.request_redraw();
        }
    }

    let menu_receiver = MenuEvent::receiver();

    let hk_proxy = proxy.clone();
    let hk_cfg = config.clone();
    let _hotkey_handle = setup_x11_hotkey(&hk_cfg, hk_proxy);

    let mut last_config_check = Instant::now();

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        // Handle tray menu events
        while let Ok(menu_event) = menu_receiver.try_recv() {
            let mut app = app.borrow_mut();
            let id = menu_event.id;

            if id == tray_menu_ids.toggle_id {
                app.visible = !app.visible;
                if app.visible {
                    app.config = Config::load();
                    app.crosshair_type = app.config.crosshair_type;
                    let cfg = app.config.clone();
                    clear_overlay_pngs(&mut app.overlays);
                    reconcile_overlays(&mut app.overlays, &cfg, &monitors, elwt);
                    show_all(&app.overlays);
                    for ov in &app.overlays {
                        ov.window.request_redraw();
                    }
                } else {
                    hide_all(&app.overlays);
                }
                cmd_tx.send(TrayCommand::SetToggleChecked(app.visible)).ok();
                continue;
            }

            if id == tray_menu_ids.exit_id {
                elwt.exit();
                continue;
            }

            if id == tray_menu_ids.save_current_id {
                let config = app.config.clone();
                app.profiles.copy_config_to_current(&config);
                app.profiles.save_to_disk();
                continue;
            }

            if id == tray_menu_ids.save_new_id {
                let name = profile_timestamp_name();
                let profile = crate::profiles::Profile::from_config(name, &app.config);
                app.profiles.list.push(profile);
                app.profiles.current = Some(app.profiles.list.len() - 1);
                app.profiles.save_to_disk();
                continue;
            }

            if id == tray_menu_ids.reload_profiles_id {
                let current_name = app.profiles.current.and_then(|i| app.profiles.list.get(i)).map(|p| p.name.clone());
                app.profiles = crate::profiles::Profiles::load();
                app.profiles.current = current_name.as_ref().and_then(|n| app.profiles.current_index_by_name(n));
                continue;
            }

            // Check crosshair type items
            let mut type_hit = None;
            for (type_idx, tid) in tray_menu_ids.type_ids.iter().enumerate() {
                if id == *tid { type_hit = Some(type_idx); break; }
            }
            if let Some(type_idx) = type_hit {
                let types = [
                    CrosshairType::Dot,
                    CrosshairType::Cross,
                    CrosshairType::T,
                    CrosshairType::Circle,
                    CrosshairType::Diamond,
                    CrosshairType::Arrow,
                ];
                if type_idx < types.len() {
                    app.crosshair_type = types[type_idx];
                    app.config.png_crosshair = None;
                    clear_overlay_pngs(&mut app.overlays);
                    if app.visible {
                        for ov in &app.overlays {
                            ov.window.request_redraw();
                        }
                    }
                }
            }

            // Check opacity items
            for (opi_idx, oid) in tray_menu_ids.opacity_ids.iter().enumerate() {
                if id == *oid && opi_idx < OPACITY_PRESETS.len() {
                    cmd_tx.send(TrayCommand::OpacityRadio(opi_idx)).ok();
                    app.config.opacity = OPACITY_PRESETS[opi_idx];
                    if app.visible {
                        for ov in &app.overlays {
                            ov.window.request_redraw();
                        }
                    }
                    break;
                }
            }

            // Check mirror toggle
            if id == tray_menu_ids.mirror_id {
                app.config.mirror_crosshair = !app.config.mirror_crosshair;
                let cfg = app.config.clone();
                reconcile_overlays(&mut app.overlays, &cfg, &monitors, elwt);
                clear_overlay_pngs(&mut app.overlays);
                if app.visible {
                    show_all(&app.overlays);
                    for ov in &app.overlays {
                        ov.window.request_redraw();
                    }
                }
                if app.config.mirror_crosshair {
                    cmd_tx.send(TrayCommand::SetMirrorChecked(true)).ok();
                    cmd_tx.send(TrayCommand::SetMonitorChecked(None)).ok();
                } else {
                    cmd_tx.send(TrayCommand::SetMirrorChecked(false)).ok();
                    cmd_tx.send(TrayCommand::SetMonitorChecked(Some(app.config.set_monitor as usize))).ok();
                }
                continue;
            }

            // Check monitor items
            for (mon_idx, mid) in tray_menu_ids.monitor_ids.iter().enumerate() {
                if id == *mid {
                    app.config.mirror_crosshair = false;
                    app.config.set_monitor = mon_idx as u32;
                    let cfg = app.config.clone();
                    reconcile_overlays(&mut app.overlays, &cfg, &monitors, elwt);
                    clear_overlay_pngs(&mut app.overlays);
                    if app.visible {
                        show_all(&app.overlays);
                        for ov in &app.overlays {
                            ov.window.request_redraw();
                        }
                    }
                    cmd_tx.send(TrayCommand::SetMirrorChecked(false)).ok();
                    cmd_tx.send(TrayCommand::SetMonitorChecked(Some(mon_idx))).ok();
                    break;
                }
            }
        }

        match event {
            Event::WindowEvent { window_id, event, .. } => {
                let mut app = app.borrow_mut();
                let ov_idx = app.overlays.iter().position(|ov| ov.window.id() == window_id);
                let Some(ov_idx) = ov_idx else { return; };

                match event {
                    WindowEvent::Resized(size) => {
                        let visible = app.visible;
                        let ov = &mut app.overlays[ov_idx];
                        ov.window_size = size;
                        ov.sw_canvas = SwCanvas::new(size.width, size.height);
                        if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
                            let _ = ov.surface.resize(w, h);
                        }
                        if visible {
                            ov.window.request_redraw();
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        let cfg = app.config.clone();
                        let ctype = app.crosshair_type;
                        let ov = &mut app.overlays[ov_idx];
                        render_overlay(ov, &cfg, ctype);
                    }
                    WindowEvent::CloseRequested => {
                        elwt.exit();
                    }
                    _ => {}
                }
            }
            Event::UserEvent(UserEvent::HotkeyPressed) => {
                let mut app = app.borrow_mut();
                app.visible = !app.visible;
                if app.visible {
                    app.config = Config::load();
                    app.crosshair_type = app.config.crosshair_type;
                    clear_overlay_pngs(&mut app.overlays);
                    show_all(&app.overlays);
                    for ov in &app.overlays {
                        ov.window.request_redraw();
                    }
                } else {
                    hide_all(&app.overlays);
                }
                cmd_tx.send(TrayCommand::SetToggleChecked(app.visible)).ok();
            }
            Event::AboutToWait => {
                let now = Instant::now();
                if now.duration_since(last_config_check) >= Duration::from_secs(2) {
                    last_config_check = now;
                    let mut app = app.borrow_mut();
                    let new_mtime = config_mtime();
                    if new_mtime != app.config_mtime {
                        app.config_mtime = new_mtime;
                        app.config = Config::load();
                        app.crosshair_type = app.config.crosshair_type;
                        let cfg = app.config.clone();
                        clear_overlay_pngs(&mut app.overlays);
                        if app.visible {
                            reconcile_overlays(&mut app.overlays, &cfg, &monitors, elwt);
                            for ov in &app.overlays {
                                ov.window.request_redraw();
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }).unwrap();
}

fn show_all(overlays: &[OverlayWindow]) {
    for ov in overlays {
        ov.window.set_visible(true);
        ov.window.set_window_level(WindowLevel::AlwaysOnTop);
    }
}

fn hide_all(overlays: &[OverlayWindow]) {
    for ov in overlays {
        ov.window.set_visible(false);
    }
}

fn clear_overlay_pngs(overlays: &mut [OverlayWindow]) {
    for ov in overlays.iter_mut() {
        ov.png_pixels = None;
    }
}

fn reconcile_overlays(
    overlays: &mut Vec<OverlayWindow>,
    config: &Config,
    monitors: &[MonitorInfo],
    elwt: &ActiveEventLoop,
) {
    let targets = determine_targets(config, monitors);
    let needed = targets.len();

    // Check if current overlays match needed config
    let same = overlays.len() == needed
        && if config.mirror_crosshair {
            overlays.iter().zip(&targets).all(|(ov, t)| ov.monitor_index == t.index)
        } else {
            overlays.len() == 1 && targets.first().map_or(false, |t| overlays[0].monitor_index == t.index)
        };

    if same {
        return;
    }

    // Need to rebuild
    // Destroy old overlays
    for ov in overlays.drain(..) {
        ov.window.set_visible(false);
    }

    let mut new_context: Option<softbuffer::Context<OwnedDisplayHandle>> = None;

    for target in &targets {
        let raw = elwt.create_window(
            Window::default_attributes()
                .with_title("ZeroIn")
                .with_visible(false)
                .with_transparent(true)
                .with_decorations(false)
                .with_active(false)
        );
        let raw = match raw {
            Ok(w) => w,
            Err(_) => continue,
        };

        raw.set_outer_position(target.position);
        let _ = raw.request_inner_size(target.size);
        let _ = raw.set_cursor_hittest(false);
        raw.set_cursor(CursorIcon::Crosshair);

        let window = Rc::new(raw);

        if new_context.is_none() {
            let display = elwt.owned_display_handle();
            new_context = Some(match softbuffer::Context::new(display) {
                Ok(c) => c,
                Err(_) => continue,
            });
        }
        let mut surface = match softbuffer::Surface::new(new_context.as_ref().unwrap(), Rc::clone(&window)) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if let (Some(w), Some(h)) = (NonZeroU32::new(target.size.width), NonZeroU32::new(target.size.height)) {
            let _ = surface.resize(w, h);
        }

        let sw_canvas = SwCanvas::new(target.size.width, target.size.height);

        overlays.push(OverlayWindow {
            window,
            surface,
            sw_canvas,
            png_pixels: None,
            window_size: target.size,
            monitor_index: target.index,
            monitor_name: target.name.clone(),
        });
    }

}
