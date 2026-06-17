use std::cell::RefCell;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use winit::dpi::PhysicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{CursorIcon, WindowLevel, Window};

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::TrayIconBuilder;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as XProtoConnectionExt;

use crate::canvas::{Canvas, SwCanvas, blend_premul};
use crate::config::{Config, CrosshairType, Hotkey};
use crate::crosshair;

const OPACITY_PRESETS: [f32; 6] = [0.1, 0.25, 0.5, 0.75, 0.9, 1.0];

struct App {
    config: Config,
    crosshair_type: CrosshairType,
    visible: bool,
    config_mtime: Option<SystemTime>,
    profiles: crate::profiles::Profiles,
    png_pixels: Option<(Vec<u8>, u32, u32)>,
    window_size: PhysicalSize<u32>,
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
    // Premultiply alpha and convert to BGRA
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
    // Try embedded icon first (compile-time)
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

    // Fallback to file lookup
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
}

enum TrayCommand {
    SetToggleChecked(bool),
    OpacityRadio(usize),
}

fn load_tray_ids_and_spawn_gtk(
    profile_names: Vec<String>,
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

            let ids = TrayMenuIds {
                toggle_id: toggle.id().clone(),
                type_ids: type_items.iter().map(|i| i.id().clone()).collect(),
                opacity_ids: opacity_items.iter().map(|i| i.id().clone()).collect(),
                save_current_id: save_current.id().clone(),
                save_new_id: save_new.id().clone(),
                reload_profiles_id: reload_profiles.id().clone(),
                exit_id: exit_item.id().clone(),
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
    // Map Windows virtual-key codes to X11 keysyms
    Some(match vk {
        0x41..=0x5A => vk as u32,                    // A-Z
        0x30..=0x39 => vk as u32,                    // 0-9
        0x20 => 0x20,                                 // Space
        0x0D => 0xFF0D,                               // Return (XK_Return)
        0x09 => 0xFF09,                               // Tab (XK_Tab)
        0x1B => 0xFF1B,                               // Escape (XK_Escape)
        0x08 => 0xFF08,                               // Backspace (XK_BackSpace)
        0x2E => 0xFFFF,                               // Delete (XK_Delete)
        0x2D => 0xFF63,                               // Insert (XK_Insert)
        0x24 => 0xFF50,                               // Home (XK_Home)
        0x23 => 0xFF57,                               // End (XK_End)
        0x21 => 0xFF55,                               // Page Up (XK_Page_Up)
        0x22 => 0xFF56,                               // Page Down (XK_Page_Down)
        0x25 => 0xFF51,                               // Left (XK_Left)
        0x27 => 0xFF53,                               // Right (XK_Right)
        0x26 => 0xFF52,                               // Up (XK_Up)
        0x28 => 0xFF54,                               // Down (XK_Down)
        0x70..=0x87 => 0xFFBE + (vk - 0x70) as u32,   // F1-F24
        0xBA => 0x3B,                                 // ;: (XK_semicolon)
        0xBB => 0x3D,                                 // =+ (XK_equal)
        0xBC => 0x2C,                                 // , (XK_comma)
        0xBD => 0x2D,                                 // - (XK_minus)
        0xBE => 0x2E,                                 // . (XK_period)
        0xBF => 0x2F,                                 // / (XK_slash)
        0xC0 => 0x60,                                 // `~ (XK_grave)
        0xDB => 0x5B,                                 // [{ (XK_bracketleft)
        0xDC => 0x5C,                                 // \| (XK_backslash)
        0xDD => 0x5D,                                 // ]} (XK_bracketright)
        0xDE => 0x27,                                 // '" (XK_apostrophe)
        0xDF => 0x5F,                                 // VK_OEM_8 varies by layout; treat as underscore
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

        // Map Win32 MOD_* to X11 modifier masks
        // MOD_ALT=1 → Mod1Mask=8, MOD_CONTROL=2 → ControlMask=4,
        // MOD_SHIFT=4 → ShiftMask=1, MOD_WIN=8 → Mod4Mask=64
        let mut x11_mod: u32 = 0;
        let m = hk.modifiers;
        if m & 0x0001 != 0 { x11_mod |= 8; }   // MOD_ALT → Mod1Mask
        if m & 0x0002 != 0 { x11_mod |= 4; }   // MOD_CONTROL → ControlMask
        if m & 0x0004 != 0 { x11_mod |= 1; }   // MOD_SHIFT → ShiftMask
        if m & 0x0008 != 0 { x11_mod |= 64; }  // MOD_WIN → Mod4Mask

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

fn render(app: &App, canvas: &mut dyn Canvas) {
    let (rr, gg, bb) = app.config.parse_color();
    let alpha = app.config.opacity;
    let main_color = (rr, gg, bb, alpha);
    let border_color = (0.0, 0.0, 0.0, 0.5 * alpha);
    let w = canvas.width() as f32;
    let h = canvas.height() as f32;
    let cx = w / 2.0;
    let cy = h / 2.0;
    let scale = 1.0; // winit provides scaled size

    crosshair::draw(
        canvas, main_color, Some(border_color),
        app.crosshair_type, cx, cy,
        app.config.size * scale,
        app.config.thickness_h * scale,
        app.config.thickness_v * scale,
        app.config.dot_center,
        app.config.border,
        app.config.border_size * scale,
        app.config.space_width * scale,
        app.config.rotation,
        app.config.dot_size,
    );
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

            // Apply opacity to premultiplied pixel
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

pub fn run() {
    let event_loop = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("ZeroIn: failed to create event loop: {e}");
            return;
        }
    };
    let proxy = event_loop.create_proxy();

    let window = match event_loop.create_window(
        Window::default_attributes()
            .with_title("ZeroIn")
            .with_visible(false)
            .with_transparent(true)
            .with_decorations(false)
            .with_active(false)
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("ZeroIn: failed to create window: {e}");
            return;
        }
    };

    // Position to cover primary monitor (not fullscreen, so set_cursor_hittest works)
    if let Some(monitor) = window.primary_monitor() {
        let pos = monitor.position();
        window.set_outer_position(pos);
        let size = monitor.size();
        let _ = window.request_inner_size(size);
    }

    // set hittest before mapping; window_level must be set after mapping
    window.set_cursor_hittest(false).unwrap();
    window.set_cursor(CursorIcon::Crosshair);

    let window_size = window.inner_size();
    let window = Rc::new(window);

    let context = match softbuffer::Context::new(Rc::clone(&window)) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("ZeroIn: failed to create softbuffer context: {e}");
            return;
        }
    };
    let mut surface = match softbuffer::Surface::new(&context, Rc::clone(&window)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ZeroIn: failed to create softbuffer surface: {e}");
            return;
        }
    };
    // Initial resize of softbuffer surface
    if let (Some(w), Some(h)) = (NonZeroU32::new(window_size.width), NonZeroU32::new(window_size.height)) {
        let _ = surface.resize(w, h);
    }

    let config = Config::load();
    let app = Rc::new(RefCell::new(App {
        config: config.clone(),
        crosshair_type: config.crosshair_type,
        visible: true,
        config_mtime: config_mtime(),
        profiles: crate::profiles::Profiles::load(),
        png_pixels: if config.png_crosshair.as_ref().map_or(false, |p| !p.is_empty()) {
            load_png_pixels(&config)
        } else {
            None
        },
        window_size,
    }));

    let profile_names: Vec<String> = app.borrow().profiles.list.iter().map(|p| p.name.clone()).collect();
    let tray_icon_bytes = try_load_tray_icon();

    // Spawn GTK thread: init GTK, create menu items, build tray icon, pump events
    let (tray_menu_ids, cmd_tx) = load_tray_ids_and_spawn_gtk(profile_names, tray_icon_bytes);

    cmd_tx.send(TrayCommand::SetToggleChecked(true)).ok();
    window.set_visible(true);
    window.set_window_level(WindowLevel::AlwaysOnTop);

    let menu_receiver = MenuEvent::receiver();

    // X11 hotkey thread (best-effort)
    let hk_proxy = proxy.clone();
    let hk_cfg = config.clone();
    let _hotkey_handle = setup_x11_hotkey(&hk_cfg, hk_proxy);

    let mut last_config_check = Instant::now();
    let mut sw_canvas = SwCanvas::new(window_size.width, window_size.height);

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        // Handle tray menu events
        while let Ok(menu_event) = menu_receiver.try_recv() {
            let mut app = app.borrow_mut();
            let id = menu_event.id;

            if id == tray_menu_ids.toggle_id {
                app.visible = !app.visible;
                if app.visible {
                    // Re-check config on show
                    app.config = Config::load();
                    app.crosshair_type = app.config.crosshair_type;
                    app.png_pixels = if app.config.png_crosshair.as_ref().map_or(false, |p| !p.is_empty()) {
                        load_png_pixels(&app.config)
                    } else {
                        None
                    };
                    window.set_visible(true);
                    window.request_redraw();
                } else {
                    window.set_visible(false);
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
                    app.png_pixels = None;
                    if app.visible { window.request_redraw(); }
                }
            }

            // Check opacity items (radio behavior: uncheck all, then check selected)
            for (opi_idx, oid) in tray_menu_ids.opacity_ids.iter().enumerate() {
                if id == *oid && opi_idx < OPACITY_PRESETS.len() {
                    cmd_tx.send(TrayCommand::OpacityRadio(opi_idx)).ok();
                    app.config.opacity = OPACITY_PRESETS[opi_idx];
                    if app.visible { window.request_redraw(); }
                    break;
                }
            }
        }

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(size) => {
                    app.borrow_mut().window_size = size;
                    sw_canvas = SwCanvas::new(size.width, size.height);
                    if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
                        let _ = surface.resize(w, h);
                    }
                    if app.borrow().visible {
                        window.request_redraw();
                    }
                }
                WindowEvent::RedrawRequested => {
                    let mut app = app.borrow_mut();
                    sw_canvas.clear();

                    let (w, h) = (sw_canvas.width(), sw_canvas.height());
                    let using_png = app.config.png_crosshair.as_ref().map_or(false, |p| !p.is_empty());

                    if using_png {
                        if app.png_pixels.is_none() {
                            app.png_pixels = load_png_pixels(&app.config);
                        }
                        if let Some((ref pixels, img_w, img_h)) = app.png_pixels.clone() {
                            let cx = w as f32 / 2.0;
                            let cy = h as f32 / 2.0;
                            let target_size = app.config.size;
                            let aspect = img_h as f32 / img_w as f32;
                            let draw_w = target_size;
                            let draw_h = target_size * aspect;
                            let rotation = app.config.rotation.to_radians();

                            if rotation != 0.0 {
                                sw_canvas.begin_rotation(rotation, cx, cy);
                            }

                            // Direct pixel blit onto SwCanvas
                            let left = cx - draw_w / 2.0;
                            let top = cy - draw_h / 2.0;
                            let opacity = app.config.opacity;
                            // This will be done via raw buffer access
                            blit_png_to_swcanvas(&mut sw_canvas, pixels, img_w, img_h,
                                                  left, top, draw_w, draw_h, opacity);

                            if rotation != 0.0 {
                                sw_canvas.end_rotation();
                            }
                        }
                    } else {
                        render(&app, &mut sw_canvas);
                    }

                    // Copy SwCanvas pixels to softbuffer surface
                    let (bw, bh) = (sw_canvas.width(), sw_canvas.height());
                    if let (Some(w), Some(h)) = (NonZeroU32::new(bw), NonZeroU32::new(bh)) {
                        let _ = surface.resize(w, h);
                    }
                    if let Ok(mut buffer) = surface.buffer_mut() {
                        let src = sw_canvas.pixels();
                        let count = buffer.len().min(src.len());
                        buffer[..count].copy_from_slice(&src[..count]);
                        if buffer.len() > src.len() {
                            buffer[src.len()..].fill(0);
                        }
                        let _ = buffer.present();
                    }
                }
                WindowEvent::CloseRequested => {
                    elwt.exit();
                }
                _ => {}
            },
            Event::UserEvent(UserEvent::HotkeyPressed) => {
                let mut app = app.borrow_mut();
                app.visible = !app.visible;
                if app.visible {
                    app.config = Config::load();
                    app.crosshair_type = app.config.crosshair_type;
                    app.png_pixels = if app.config.png_crosshair.as_ref().map_or(false, |p| !p.is_empty()) {
                        load_png_pixels(&app.config)
                    } else {
                        None
                    };
                    window.set_visible(true);
                    window.request_redraw();
                } else {
                    window.set_visible(false);
                }
                cmd_tx.send(TrayCommand::SetToggleChecked(app.visible)).ok();
            }
            Event::AboutToWait => {
                // Config polling every 2 seconds
                let now = Instant::now();
                if now.duration_since(last_config_check) >= Duration::from_secs(2) {
                    last_config_check = now;
                    let mut app = app.borrow_mut();
                    let new_mtime = config_mtime();
                    if new_mtime != app.config_mtime {
                        app.config_mtime = new_mtime;
                        app.config = Config::load();
                        app.crosshair_type = app.config.crosshair_type;
                        app.png_pixels = if app.config.png_crosshair.as_ref().map_or(false, |p| !p.is_empty()) {
                            load_png_pixels(&app.config)
                        } else {
                            None
                        };
                        if app.visible {
                            window.request_redraw();
                        }
                    }
                }
            }
            _ => {}
        }
    }).unwrap();
}
