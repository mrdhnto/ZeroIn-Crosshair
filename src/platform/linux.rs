use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use winit::dpi::PhysicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Fullscreen, WindowLevel, Window};

use softbuffer::Surface;

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::TrayIconBuilder;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as XProtoConnectionExt;
use x11rb::protocol::shape::ConnectionExt as ShapeConnectionExt;

use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

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

struct TrayMenuSet {
    toggle: CheckMenuItem,
    type_items: Vec<MenuItem>,
    opacity_items: Vec<CheckMenuItem>,
    profile_items: Vec<MenuItem>,
    save_current: MenuItem,
    save_new: MenuItem,
    reload_profiles: MenuItem,
    exit_item: MenuItem,
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

fn build_tray_menu(app: &Rc<RefCell<App>>) -> (TrayMenuSet, Menu) {
    let menu = Menu::new();

    let toggle = CheckMenuItem::new("Toggle On/Off", true, false, None);
    menu.append(&toggle).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();

    let type_submenu = Submenu::new("Crosshair Type", true);
    let type_names = ["Dot", "Cross", "T", "Circle", "Diamond", "Arrow"];
    let type_items: Vec<MenuItem> = type_names.iter().map(|&n| {
        let item = MenuItem::new(n, true, None);
        type_submenu.append(&item).unwrap();
        item
    }).collect();
    menu.append(&type_submenu).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();

    let opacity_submenu = Submenu::new("Opacity", true);
    let opacity_items: Vec<CheckMenuItem> = OPACITY_PRESETS.iter().map(|&val| {
        let label = format!("{:.2}", val);
        let item = CheckMenuItem::new(&label, true, false, None);
        opacity_submenu.append(&item).unwrap();
        item
    }).collect();
    menu.append(&opacity_submenu).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();

    let profiles_submenu = Submenu::new("Profiles", true);
    let profile_items: Vec<MenuItem> = Vec::new(); // populated on rebuild
    let save_current = MenuItem::new("Save Current", true, None);
    let save_new = MenuItem::new("Save As New Profile", true, None);
    let reload_profiles = MenuItem::new("Reload Profiles", true, None);
    profiles_submenu.append(&save_current).unwrap();
    profiles_submenu.append(&save_new).unwrap();
    profiles_submenu.append(&reload_profiles).unwrap();
    menu.append(&profiles_submenu).unwrap();
    menu.append(&PredefinedMenuItem::separator()).unwrap();

    let exit_item = MenuItem::new("Exit", true, None);
    menu.append(&exit_item).unwrap();

    // Rebuild profile list in profiles_submenu
    let app = app.borrow();
    let items = menu.items();
    let profiles_menu_ref = items.last().unwrap().as_submenu().unwrap();
    // Insert profile items before Save Current
    for (i, profile) in app.profiles.list.iter().enumerate() {
        let item = MenuItem::new(&profile.name, true, None);
        // Insert at position (before Save Current)
        profiles_menu_ref.insert(&item, i).unwrap();
    }

    (TrayMenuSet {
        toggle,
        type_items,
        opacity_items,
        profile_items,
        save_current,
        save_new,
        reload_profiles,
        exit_item,
    }, menu)
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

        let keycode = {
            let key_str = hk.vk;
            let keysym = match key_str as u8 {
                b'A'..=b'Z' => key_str as u32,
                b'0'..=b'9' => 0x30 + (u32::from(key_str) - u32::from(b'0')),
                _ => return,
            };
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
            .with_fullscreen(Some(Fullscreen::Borderless(None)))
            .with_transparent(true)
            .with_decorations(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("ZeroIn: failed to create window: {e}");
            return;
        }
    };

    let window_size = window.inner_size();
    let window = Rc::new(window);

    let context = match unsafe { softbuffer::Context::new(Rc::clone(&window)) } {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("ZeroIn: failed to create softbuffer context: {e}");
            return;
        }
    };
    let mut surface = match unsafe { Surface::new(&context, Rc::clone(&window)) } {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ZeroIn: failed to create softbuffer surface: {e}");
            return;
        }
    };

    let config = Config::load();
    let app = Rc::new(RefCell::new(App {
        config: config.clone(),
        crosshair_type: config.crosshair_type,
        visible: false,
        config_mtime: config_mtime(),
        profiles: crate::profiles::Profiles::load(),
        png_pixels: if config.png_crosshair.as_ref().map_or(false, |p| !p.is_empty()) {
            load_png_pixels(&config)
        } else {
            None
        },
        window_size,
    }));

    let tray_icon = try_load_tray_icon();
    let mut tray_builder = TrayIconBuilder::new().with_tooltip("ZeroIn Crosshair");
    if let Some(icon) = tray_icon {
        tray_builder = tray_builder.with_icon(icon);
    }

    let (tray_menu_set, tray_menu) = build_tray_menu(&app);
    tray_builder = tray_builder.with_menu(Box::new(tray_menu));
    let _tray_icon = tray_builder.build();
    let menu_receiver = MenuEvent::receiver();

    // Make window click-through on X11 via Shape extension
    if let Ok(handle) = window.window_handle() {
        let raw = handle.as_raw();
        let xid = match raw {
            RawWindowHandle::Xlib(h) => h.window,
            RawWindowHandle::Xcb(h) => h.window.get() as u64,
            _ => 0,
        };
        if xid != 0 {
            if let Ok((shape_conn, _)) = x11rb::connect(None) {
                let _ = shape_conn.shape_rectangles(
                    x11rb::protocol::shape::SO::SET,
                    x11rb::protocol::shape::SK::INPUT,
                    x11rb::protocol::xproto::ClipOrdering::UNSORTED,
                    xid as u32,
                    0i16, 0i16,
                    &[],
                );
                let _ = shape_conn.flush();
            }
        }
    }

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

            if id == tray_menu_set.toggle.id() {
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
                tray_menu_set.toggle.set_checked(app.visible);
                continue;
            }

            if id == tray_menu_set.exit_item.id() {
                elwt.exit();
                continue;
            }

            if id == tray_menu_set.save_current.id() {
                let config = app.config.clone();
                app.profiles.copy_config_to_current(&config);
                app.profiles.save_to_disk();
                continue;
            }

            if id == tray_menu_set.save_new.id() {
                let name = profile_timestamp_name();
                let profile = crate::profiles::Profile::from_config(name, &app.config);
                app.profiles.list.push(profile);
                app.profiles.current = Some(app.profiles.list.len() - 1);
                app.profiles.save_to_disk();
                continue;
            }

            if id == tray_menu_set.reload_profiles.id() {
                let current_name = app.profiles.current.and_then(|i| app.profiles.list.get(i)).map(|p| p.name.clone());
                app.profiles = crate::profiles::Profiles::load();
                app.profiles.current = current_name.as_ref().and_then(|n| app.profiles.current_index_by_name(n));
                continue;
            }

            // Check crosshair type items
            for (type_idx, item) in tray_menu_set.type_items.iter().enumerate() {
                if id == item.id() {
                    // Map index to CrosshairType
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
                    break;
                }
            }

            // Check opacity items
            for (opi_idx, item) in tray_menu_set.opacity_items.iter().enumerate() {
                if id == item.id() {
                    if opi_idx < OPACITY_PRESETS.len() {
                        app.config.opacity = OPACITY_PRESETS[opi_idx];
                        if app.visible { window.request_redraw(); }
                    }
                    break;
                }
            }
        }

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(size) => {
                    app.borrow_mut().window_size = size;
                    sw_canvas = SwCanvas::new(size.width, size.height);
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
                tray_menu_set.toggle.set_checked(app.visible);
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
