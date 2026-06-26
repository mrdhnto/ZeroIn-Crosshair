use core::ffi::c_void;
use std::mem;
use std::time::SystemTime;

use windows::core::*;
use windows::Foundation::Numerics::Matrix3x2;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
    D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BITMAP_PROPERTIES,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
    D2D1_RENDER_TARGET_USAGE_NONE, ID2D1Bitmap, ID2D1DCRenderTarget, ID2D1Factory, ID2D1RenderTarget,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT, SetProcessDpiAwarenessContext};
use windows::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use notify::{Config as NotifyConfig, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify::event::ModifyKind;

use windows::Win32::System::Threading::GetCurrentThreadId;

use crate::canvas::D2DCanvas;
use crate::config::{Config, CrosshairType, Hotkey};

const WM_TRAYICON: u32 = WM_APP + 1;
const WM_CONFIG_CHANGED: u32 = WM_APP + 2;
const WM_HOTKEY_MSG: u32 = 0x0312;
const WM_CLOSE_MSG: u32 = 0x0010;
const HOTKEY_ID: i32 = 9001;

const IDM_TOGGLE: u16 = 1001;
const IDM_TYPE_DOT: u16 = 1002;
const IDM_TYPE_CROSS: u16 = 1003;
const IDM_TYPE_T: u16 = 1004;
const IDM_TYPE_CIRCLE: u16 = 1006;
const IDM_TYPE_DIAMOND: u16 = 1008;
const IDM_TYPE_ARROW: u16 = 1009;
const IDM_EXIT: u16 = 1005;
const IDM_OPACITY_BASE: u16 = 2001;
const IDM_PROFILES_BASE: u16 = 2100;
const IDM_PROFILE_SAVE: u16 = 2201;
const IDM_PROFILE_SAVE_NEW: u16 = 2202;
const IDM_PROFILE_RELOAD: u16 = 2203;

const IDM_MIRROR_TOGGLE: u16 = 3001;
const IDM_MONITOR_BASE: u16 = 3100;

const OPACITY_PRESETS: [f32; 6] = [0.1, 0.25, 0.5, 0.75, 0.9, 1.0];

struct OverlayWindow {
    hwnd: HWND,
    rt: Option<RenderResources>,
    png_bitmap: Option<windows::Win32::Graphics::Direct2D::ID2D1Bitmap>,
    monitor_index: u32,
    monitor_rect: RECT,
}

struct App {
    overlays: Vec<OverlayWindow>,
    factory: ID2D1Factory,
    config: Config,
    crosshair_type: CrosshairType,
    visible: bool,
    custom_icon: Option<HICON>,
    config_mtime: Option<std::time::SystemTime>,
    profiles: crate::profiles::Profiles,
    tray_hwnd: HWND,
}

struct RenderResources {
    target: ID2D1DCRenderTarget,
    mem_dc: HDC,
    dib: HBITMAP,
    width: u32,
    height: u32,
}

struct MonitorInfo {
    index: u32,
    rect: RECT,
    name: String,
}

// ── Icon loading ────────────────────────────────────────────

fn load_custom_icon() -> Option<HICON> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let icon_path = exe_dir.join("icon.ico");
    if !icon_path.exists() {
        return None;
    }
    let wstr = WStr::new(&icon_path.to_string_lossy());
    unsafe {
        match LoadImageW(
            HINSTANCE::default(),
            wstr.as_pcwstr(),
            GDI_IMAGE_TYPE(1),
            0,
            0,
            IMAGE_FLAGS(0x10),
        ) {
            Ok(h) if h != HANDLE::default() => Some(mem::transmute(h)),
            _ => None,
        }
    }
}

fn load_embedded_icon(hinstance: HINSTANCE) -> Option<HICON> {
    unsafe {
        let id = 1usize as *const u16;
        match LoadImageW(
            hinstance,
            PCWSTR(id),
            GDI_IMAGE_TYPE(1),
            0,
            0,
            IMAGE_FLAGS(0),
        ) {
            Ok(h) if h != HANDLE::default() => Some(mem::transmute(h)),
            _ => None,
        }
    }
}

struct WStr(Vec<u16>);
impl WStr {
    fn new(s: &str) -> Self {
        Self(s.encode_utf16().chain(Some(0)).collect())
    }
    fn as_pcwstr(&self) -> PCWSTR {
        PCWSTR(self.0.as_ptr())
    }
}

pub fn run() {
    if let Err(e) = run_impl() {
        let _ = crate::config::log_warning(&format!("ZeroIn error: {e}"));
    }
}

fn run_impl() -> Result<()> {
    unsafe {
        let ctx = DPI_AWARENESS_CONTEXT((-4isize) as *mut c_void);
        if SetProcessDpiAwarenessContext(ctx).is_err() {
            let _ = SetProcessDPIAware();
        }
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok();

        let hmodule = GetModuleHandleW(None)?;
        let hinstance: HINSTANCE = mem::transmute(hmodule);
        let cfg = Config::load();

        let custom_icon = load_custom_icon().or_else(|| load_embedded_icon(hinstance));
        let default_icon = unsafe { LoadIconW(None, IDI_APPLICATION)? };
        let icon = custom_icon.unwrap_or(default_icon);

        let tc = WStr::new("CrosshairTrayCls");
        let oc = WStr::new("CrosshairOverlayCls");

        RegisterClassW(&WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(tray_wndproc),
            hInstance: hinstance,
            hIcon: icon,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH::default(),
            lpszClassName: tc.as_pcwstr(),
            ..Default::default()
        });

        RegisterClassW(&WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(overlay_wndproc),
            hInstance: hinstance,
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH::default(),
            lpszClassName: oc.as_pcwstr(),
            ..Default::default()
        });

        let tray_hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            tc.as_pcwstr(),
            WStr::new("CrosshairOverlay").as_pcwstr(),
            WS_OVERLAPPEDWINDOW,
            0, 0, 0, 0,
            None, None, hinstance, None,
        )?;

        let factory: ID2D1Factory =
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;

        let overlays = create_overlay_windows(&oc, hinstance, &cfg)?;
        for ov in &overlays {
            unsafe { ShowWindow(ov.hwnd, SW_HIDE) };
        }

        let mut app = Box::new(App {
            overlays,
            factory,
            config: cfg.clone(),
            crosshair_type: cfg.crosshair_type,
            visible: false,
            custom_icon,
            config_mtime: config_mtime_of(&cfg),
            profiles: crate::profiles::Profiles::load(),
            tray_hwnd,
        });

        let app_ptr: *mut App = &mut *app;
        for ov in &app.overlays {
            unsafe { SetWindowLongPtrW(ov.hwnd, GWLP_USERDATA, app_ptr as isize) };
        }
        unsafe { SetWindowLongPtrW(tray_hwnd, GWLP_USERDATA, app_ptr as isize) };

        create_tray_icon(tray_hwnd, icon)?;

        if let Some(hk) = Hotkey::from_parts(&cfg.primary_key, &cfg.secondary_key) {
            let _ = RegisterHotKey(
                tray_hwnd,
                HOTKEY_ID,
                HOT_KEY_MODIFIERS(hk.modifiers),
                hk.vk as u32,
            );
        }

        // Set up file watcher for config.ini
        // Watch the parent directory, not the file, to survive atomic saves
        // (editors rename temp → target, changing the inode)
        let config_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let tid = unsafe { GetCurrentThreadId() };
        let mut watcher: RecommendedWatcher = Watcher::new(
            move |event: std::result::Result<notify::Event, notify::Error>| {
                if let Ok(event) = event {
                    let is_config = event.paths.iter().any(|p| p.ends_with("config.ini"));
                    if !is_config { return; }
                    match event.kind {
                        EventKind::Modify(ModifyKind::Data(_))
                        | EventKind::Modify(ModifyKind::Name(_))
                        | EventKind::Create(_) => {
                            unsafe { let _ = PostThreadMessageW(tid, WM_CONFIG_CHANGED, WPARAM(0), LPARAM(0)); }
                        }
                        _ => {}
                    }
                }
            },
            NotifyConfig::default(),
        ).unwrap();
        if watcher.watch(&config_dir, RecursiveMode::NonRecursive).is_err() {
            let _ = crate::config::log_warning("ZeroIn: failed to watch config directory");
        }
        Box::leak(Box::new(watcher));

        // Polling fallback: 2s timer checks mtime in case watcher misses events
        let _ = unsafe { SetTimer(tray_hwnd, 1, 2000, None); };

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        drop_tray_icon(tray_hwnd);
        destroy_overlays(&mut app.overlays);
        drop(app);
        CoUninitialize();
        Ok(())
    }
}

// ── Helpers ─────────────────────────────────────────────────

unsafe fn app_mut(hwnd: HWND) -> &'static mut App {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App };
    unsafe { &mut *ptr }
}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = unsafe { &mut *(lparam.0 as *mut Vec<MonitorInfo>) };
    let mut mi = unsafe { mem::zeroed::<MONITORINFO>() };
    mi.cbSize = mem::size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(hmonitor, &mut mi).as_bool() } {
        let idx = monitors.len() as u32;
        monitors.push(MonitorInfo {
            index: idx,
            rect: mi.rcMonitor,
            name: format!("Monitor {idx}"),
        });
    }
    BOOL(1)
}

fn enumerate_monitors() -> Vec<MonitorInfo> {
    let mut monitors = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM(&mut monitors as *mut Vec<MonitorInfo> as isize),
        );
    }
    monitors
}

fn create_overlay_windows(
    oc: &WStr,
    hinstance: HINSTANCE,
    cfg: &Config,
) -> Result<Vec<OverlayWindow>> {
    let monitors = enumerate_monitors();
    if monitors.is_empty() {
        return Ok(Vec::new());
    }

    let targets: Vec<&MonitorInfo> = if cfg.mirror_crosshair {
        monitors.iter().collect()
    } else {
        let idx = (cfg.set_monitor as usize).min(monitors.len().saturating_sub(1));
        vec![&monitors[idx]]
    };

    let mut overlays = Vec::with_capacity(targets.len());
    for &mon in &targets {
        let ow = mon.rect.right - mon.rect.left;
        let oh = mon.rect.bottom - mon.rect.top;
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                oc.as_pcwstr(),
                WStr::new("").as_pcwstr(),
                WS_POPUP,
                mon.rect.left, mon.rect.top, ow, oh,
                None, None, hinstance, None,
            )
        }?;
        overlays.push(OverlayWindow {
            hwnd,
            rt: None,
            png_bitmap: None,
            monitor_index: mon.index,
            monitor_rect: mon.rect,
        });
    }
    Ok(overlays)
}

fn destroy_overlays(overlays: &mut Vec<OverlayWindow>) {
    for ov in overlays.iter_mut() {
        if let Some(rt) = ov.rt.take() {
            if rt.width > 0 {
                unsafe { SelectObject(rt.mem_dc, HGDIOBJ(rt.dib.0 as *mut core::ffi::c_void)) };
            }
            unsafe { DeleteObject(HGDIOBJ(rt.dib.0 as *mut core::ffi::c_void)) };
            unsafe { DeleteDC(rt.mem_dc) };
        }
        if !ov.hwnd.is_invalid() {
            unsafe { DestroyWindow(ov.hwnd) };
        }
    }
    overlays.clear();
}

fn show_overlays(overlays: &[OverlayWindow]) {
    for ov in overlays {
        unsafe {
            ShowWindow(ov.hwnd, SW_SHOW);
            SetWindowPos(
                ov.hwnd,
                HWND_TOPMOST,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
        }
    }
}

fn hide_overlays(overlays: &[OverlayWindow]) {
    for ov in overlays {
        unsafe { ShowWindow(ov.hwnd, SW_HIDE) };
    }
}

fn clear_overlay_bitmaps(overlays: &mut [OverlayWindow]) {
    for ov in overlays.iter_mut() {
        ov.png_bitmap = None;
    }
}

fn reconcile_overlays(app: &mut App) {
    let oc_name = WStr::new("CrosshairOverlayCls");
    let monitors = enumerate_monitors();
    if monitors.is_empty() { return; }

    let needed_count = if app.config.mirror_crosshair {
        monitors.len()
    } else {
        1usize
    };

    // If count matches and monitor indices match, keep current windows
    let same = app.overlays.len() == needed_count
        && if app.config.mirror_crosshair {
            app.overlays.iter().zip(&monitors).all(|(ov, m)| ov.monitor_index == m.index)
        } else {
            let idx = (app.config.set_monitor as usize).min(monitors.len().saturating_sub(1));
            app.overlays.len() == 1 && app.overlays[0].monitor_index == monitors[idx].index
        };

    if same {
        // Just reposition existing windows
        for ov in &mut app.overlays {
            if let Some(m) = monitors.iter().find(|m| m.index == ov.monitor_index) {
                ov.monitor_rect = m.rect;
                unsafe {
                    SetWindowPos(
                        ov.hwnd,
                        HWND_TOPMOST,
                        m.rect.left, m.rect.top,
                        m.rect.right - m.rect.left,
                        m.rect.bottom - m.rect.top,
                        SWP_NOZORDER,
                    );
                }
            }
        }
        return;
    }

    // Need to recreate overlays
    unsafe {
        let hinstance: HINSTANCE = mem::transmute(GetModuleHandleW(None).unwrap_or(HMODULE::default()));
        destroy_overlays(&mut app.overlays);

        // Re-register overlay window class in case it was unregistered
        let _ = RegisterClassW(&WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(overlay_wndproc),
            hInstance: hinstance,
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or(HCURSOR::default()),
            hbrBackground: HBRUSH::default(),
            lpszClassName: oc_name.as_pcwstr(),
            ..Default::default()
        });

        let app_ptr = &mut *app as *mut App;
        let new_overlays = create_overlay_windows(&oc_name, hinstance, &app.config).unwrap_or_default();
        for ov in &new_overlays {
            SetWindowLongPtrW(ov.hwnd, GWLP_USERDATA, app_ptr as isize);
        }
        app.overlays = new_overlays;
    }
}

fn config_mtime_of(_cfg: &Config) -> Option<SystemTime> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let config_path = exe_dir.join("config.ini");
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

unsafe fn png_load_bitmap(
    config: &Config,
    factory: &ID2D1Factory,
    target: &ID2D1DCRenderTarget,
) -> Option<ID2D1Bitmap> {
    let path = match &config.png_crosshair {
        Some(p) => p,
        None => return None,
    };
    let path = std::path::Path::new(path);
    if !path.exists() {
        return None;
    }
    let img = match image::ImageReader::open(path) {
        Ok(r) => match r.decode() {
            Ok(i) => i.to_rgba8(),
            Err(_) => return None,
        },
        Err(_) => return None,
    };
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return None;
    }

    let pixels: Vec<u8> = img.chunks(4).flat_map(|p| {
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

    let size = D2D_SIZE_U { width: w, height: h };
    let props = D2D1_BITMAP_PROPERTIES {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
    };

    unsafe {
        target.CreateBitmap(
            size,
            Some(pixels.as_ptr() as *const c_void),
            4 * w,
            &props as *const D2D1_BITMAP_PROPERTIES,
        ).ok()
    }
}

// ── Tray icon ───────────────────────────────────────────────

unsafe fn create_tray_icon(hwnd: HWND, icon: HICON) -> Result<()> {
    let mut nid: NOTIFYICONDATAW = unsafe { mem::zeroed() };
    nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = icon;
    let tip = WStr::new("ZeroIn Crosshair");
    for (i, &c) in tip.0.iter().enumerate().take(128) {
        nid.szTip[i] = c;
    }

    if unsafe { Shell_NotifyIconW(NIM_ADD, &nid) }.as_bool() {
        Ok(())
    } else {
        Err(Error::from_win32())
    }
}

unsafe fn drop_tray_icon(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = unsafe { mem::zeroed() };
    nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
}

unsafe fn show_tray_menu(hwnd: HWND, app: &App) {
    let hmenu = unsafe { CreatePopupMenu() }.unwrap();
    let hsub_type = unsafe { CreatePopupMenu() }.unwrap();

    unsafe {
        AppendMenuW(hsub_type, MENU_ITEM_FLAGS(0), IDM_TYPE_DOT as usize, WStr::new("Dot").as_pcwstr());
        AppendMenuW(hsub_type, MENU_ITEM_FLAGS(0), IDM_TYPE_CROSS as usize, WStr::new("Cross").as_pcwstr());
        AppendMenuW(hsub_type, MENU_ITEM_FLAGS(0), IDM_TYPE_T as usize, WStr::new("T").as_pcwstr());
        AppendMenuW(hsub_type, MENU_ITEM_FLAGS(0), IDM_TYPE_CIRCLE as usize, WStr::new("Circle").as_pcwstr());
        AppendMenuW(hsub_type, MENU_ITEM_FLAGS(0), IDM_TYPE_DIAMOND as usize, WStr::new("Diamond").as_pcwstr());
        AppendMenuW(hsub_type, MENU_ITEM_FLAGS(0), IDM_TYPE_ARROW as usize, WStr::new("Arrow").as_pcwstr());
    }

    match app.crosshair_type {
        CrosshairType::Dot => unsafe { CheckMenuItem(hsub_type, IDM_TYPE_DOT as u32, MF_CHECKED.0) },
        CrosshairType::Cross => unsafe { CheckMenuItem(hsub_type, IDM_TYPE_CROSS as u32, MF_CHECKED.0) },
        CrosshairType::T => unsafe { CheckMenuItem(hsub_type, IDM_TYPE_T as u32, MF_CHECKED.0) },
        CrosshairType::Circle => unsafe { CheckMenuItem(hsub_type, IDM_TYPE_CIRCLE as u32, MF_CHECKED.0) },
        CrosshairType::Diamond => unsafe { CheckMenuItem(hsub_type, IDM_TYPE_DIAMOND as u32, MF_CHECKED.0) },
        CrosshairType::Arrow => unsafe { CheckMenuItem(hsub_type, IDM_TYPE_ARROW as u32, MF_CHECKED.0) },
    };

    let hsub_opacity = unsafe { CreatePopupMenu() }.unwrap();
    let closest = closest_opacity_idx(app.config.opacity);
    for (i, &val) in OPACITY_PRESETS.iter().enumerate() {
        let flags = if i == closest { MF_CHECKED } else { MENU_ITEM_FLAGS(0) };
        let label = format!("{:.2}", val);
        unsafe {
            AppendMenuW(hsub_opacity, flags, (IDM_OPACITY_BASE + i as u16) as usize, WStr::new(&label).as_pcwstr());
        }
    }

    let hsub_profiles = unsafe { CreatePopupMenu() }.unwrap();
    for (i, profile) in app.profiles.list.iter().enumerate() {
        let flags = if Some(i) == app.profiles.current { MF_CHECKED } else { MENU_ITEM_FLAGS(0) };
        unsafe {
            AppendMenuW(hsub_profiles, flags, (IDM_PROFILES_BASE + i as u16) as usize, WStr::new(&profile.name).as_pcwstr());
        }
    }
    unsafe {
        AppendMenuW(hsub_profiles, MENU_ITEM_FLAGS(MF_SEPARATOR.0), 0, None);
        AppendMenuW(hsub_profiles, MENU_ITEM_FLAGS(0), IDM_PROFILE_SAVE as usize, WStr::new("Save Current").as_pcwstr());
        AppendMenuW(hsub_profiles, MENU_ITEM_FLAGS(0), IDM_PROFILE_SAVE_NEW as usize, WStr::new("Save As New Profile").as_pcwstr());
        AppendMenuW(hsub_profiles, MENU_ITEM_FLAGS(0), IDM_PROFILE_RELOAD as usize, WStr::new("Reload Profiles").as_pcwstr());
    }

    let toggle_flags = MENU_ITEM_FLAGS(MF_STRING.0)
        | if app.visible { MF_CHECKED } else { MENU_ITEM_FLAGS(0) };

    unsafe {
        AppendMenuW(hmenu, toggle_flags, IDM_TOGGLE as usize, WStr::new("Toggle On/Off").as_pcwstr());
        AppendMenuW(hmenu, MENU_ITEM_FLAGS(MF_SEPARATOR.0), 0, None);
        AppendMenuW(
            hmenu,
            MENU_ITEM_FLAGS(MF_POPUP.0 | MF_STRING.0),
            mem::transmute::<HMENU, usize>(hsub_type),
            WStr::new("Crosshair Type").as_pcwstr(),
        );
        AppendMenuW(
            hmenu,
            MENU_ITEM_FLAGS(MF_POPUP.0 | MF_STRING.0),
            mem::transmute::<HMENU, usize>(hsub_opacity),
            WStr::new("Opacity").as_pcwstr(),
        );
        let hsub_monitor = unsafe { CreatePopupMenu() }.unwrap();
        {
            let mirror_flags = MENU_ITEM_FLAGS(if app.config.mirror_crosshair { MF_CHECKED.0 } else { 0 });
            unsafe {
                AppendMenuW(hsub_monitor, mirror_flags, IDM_MIRROR_TOGGLE as usize, WStr::new("Mirror All Monitors").as_pcwstr());
                AppendMenuW(hsub_monitor, MENU_ITEM_FLAGS(MF_SEPARATOR.0), 0, None);
            }
            let monitors = enumerate_monitors();
            for mon in &monitors {
                let selected = !app.config.mirror_crosshair && mon.index == app.config.set_monitor;
                let flags = MENU_ITEM_FLAGS(if selected { MF_CHECKED.0 } else { 0 });
                unsafe {
                    AppendMenuW(hsub_monitor, flags, (IDM_MONITOR_BASE + mon.index as u16) as usize, WStr::new(&mon.name).as_pcwstr());
                }
            }
        }
        unsafe {
            AppendMenuW(
                hmenu,
                MENU_ITEM_FLAGS(MF_POPUP.0 | MF_STRING.0),
                mem::transmute::<HMENU, usize>(hsub_monitor),
                WStr::new("Monitor").as_pcwstr(),
            );
        }
        unsafe {
            AppendMenuW(hmenu, MENU_ITEM_FLAGS(MF_SEPARATOR.0), 0, None);
            AppendMenuW(
                hmenu,
                MENU_ITEM_FLAGS(MF_POPUP.0 | MF_STRING.0),
                mem::transmute::<HMENU, usize>(hsub_profiles),
                WStr::new("Profiles").as_pcwstr(),
            );
            AppendMenuW(hmenu, MENU_ITEM_FLAGS(MF_SEPARATOR.0), 0, None);
            AppendMenuW(hmenu, MENU_ITEM_FLAGS(0), IDM_EXIT as usize, WStr::new("Exit").as_pcwstr());
        }
    }

    let mut pt = POINT::default();
    unsafe { GetCursorPos(&mut pt) };
    unsafe { SetForegroundWindow(hwnd) };
    unsafe {
        TrackPopupMenu(
            hmenu,
            TRACK_POPUP_MENU_FLAGS(TPM_LEFTALIGN.0 | TPM_RIGHTBUTTON.0),
            pt.x, pt.y, 0, hwnd, None,
        )
    };
    unsafe { PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0)) };

    unsafe { DestroyMenu(hmenu) };
}

// ── Window procedures ───────────────────────────────────────

unsafe extern "system" fn tray_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { match msg {
        WM_TRAYICON => {
            let app = app_mut(hwnd);
            let msg_id = lparam.0 as u16;
            if msg_id == 0x202 || msg_id == 0x205 {
                show_tray_menu(hwnd, app);
            }
            LRESULT(0)
        }
        WM_CONFIG_CHANGED => {
            let app = app_mut(hwnd);
            let new_mtime = config_mtime_of(&app.config);
            if new_mtime != app.config_mtime {
                app.config_mtime = new_mtime;
                crate::profiles::load_config_with_active_profile(&mut app.config, &app.profiles);
                app.crosshair_type = app.config.crosshair_type;
                clear_overlay_bitmaps(&mut app.overlays);
                if app.visible {
                    let _ = render(app);
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            // Polling fallback: check mtime every 2s for changes watcher missed
            // (atomic saves, network drives, etc.)
            let app = app_mut(hwnd);
            let new_mtime = config_mtime_of(&app.config);
            if new_mtime != app.config_mtime {
                app.config_mtime = new_mtime;
                crate::profiles::load_config_with_active_profile(&mut app.config, &app.profiles);
                app.crosshair_type = app.config.crosshair_type;
                clear_overlay_bitmaps(&mut app.overlays);
                if app.visible {
                    let _ = render(app);
                }
            }
            LRESULT(0)
        }
        WM_HOTKEY_MSG => {
            let app = app_mut(hwnd);
            app.visible = !app.visible;
            if app.visible {
                crate::profiles::load_config_with_active_profile(&mut app.config, &app.profiles);
                app.crosshair_type = app.config.crosshair_type;
                reconcile_overlays(app);
                clear_overlay_bitmaps(&mut app.overlays);
                show_overlays(&app.overlays);
                let _ = render(app);
            } else {
                hide_overlays(&app.overlays);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let app = app_mut(hwnd);
            let id = wparam.0 as u16;
            match id {
                IDM_TOGGLE => {
                    app.visible = !app.visible;
                    if app.visible {
                        crate::profiles::load_config_with_active_profile(&mut app.config, &app.profiles);
                        app.crosshair_type = app.config.crosshair_type;
                        reconcile_overlays(app);
                        clear_overlay_bitmaps(&mut app.overlays);
                        show_overlays(&app.overlays);
                        let _ = render(app);
                    } else {
                        hide_overlays(&app.overlays);
                    }
                }
                IDM_TYPE_DOT => {
                    app.crosshair_type = CrosshairType::Dot;
                    app.config.png_crosshair = None;
                    if app.visible { let _ = render(app); }
                }
                IDM_TYPE_CROSS => {
                    app.crosshair_type = CrosshairType::Cross;
                    app.config.png_crosshair = None;
                    if app.visible { let _ = render(app); }
                }
                IDM_TYPE_T => {
                    app.crosshair_type = CrosshairType::T;
                    app.config.png_crosshair = None;
                    if app.visible { let _ = render(app); }
                }
                IDM_TYPE_CIRCLE => {
                    app.crosshair_type = CrosshairType::Circle;
                    app.config.png_crosshair = None;
                    if app.visible { let _ = render(app); }
                }
                IDM_TYPE_DIAMOND => {
                    app.crosshair_type = CrosshairType::Diamond;
                    app.config.png_crosshair = None;
                    if app.visible { let _ = render(app); }
                }
                IDM_TYPE_ARROW => {
                    app.crosshair_type = CrosshairType::Arrow;
                    app.config.png_crosshair = None;
                    if app.visible { let _ = render(app); }
                }
                id if id >= IDM_OPACITY_BASE && id < IDM_OPACITY_BASE + OPACITY_PRESETS.len() as u16 => {
                    let idx = (id - IDM_OPACITY_BASE) as usize;
                    app.config.opacity = OPACITY_PRESETS[idx];
                    if app.visible { let _ = render(app); }
                }
                id if id >= IDM_PROFILES_BASE && id < IDM_PROFILES_BASE + app.profiles.list.len() as u16 => {
                    let idx = (id - IDM_PROFILES_BASE) as usize;
                    app.profiles.current = Some(idx);
                    app.profiles.apply_to_config(&mut app.config, idx);
                    app.crosshair_type = app.config.crosshair_type;
                    clear_overlay_bitmaps(&mut app.overlays);
                    if app.visible { let _ = render(app); }
                }
                IDM_PROFILE_SAVE => {
                    app.profiles.copy_config_to_current(&app.config);
                    app.profiles.save_to_disk();
                }
                IDM_PROFILE_SAVE_NEW => {
                    let name = profile_timestamp_name();
                    let profile = crate::profiles::Profile::from_config(name, &app.config);
                    app.profiles.list.push(profile);
                    app.profiles.current = Some(app.profiles.list.len() - 1);
                    app.profiles.save_to_disk();
                }
                IDM_PROFILE_RELOAD => {
                    let current_name = app.profiles.current.and_then(|i| app.profiles.list.get(i)).map(|p| p.name.clone());
                    app.profiles = crate::profiles::Profiles::load();
                    app.profiles.current = current_name.as_ref().and_then(|n| app.profiles.current_index_by_name(n));
                    if let Some(idx) = app.profiles.current {
                        app.profiles.apply_to_config(&mut app.config, idx);
                        app.crosshair_type = app.config.crosshair_type;
                        clear_overlay_bitmaps(&mut app.overlays);
                        if app.visible { let _ = render(app); }
                    }
                }
                IDM_MIRROR_TOGGLE => {
                    app.config.mirror_crosshair = !app.config.mirror_crosshair;
                    reconcile_overlays(app);
                    clear_overlay_bitmaps(&mut app.overlays);
                    if app.visible {
                        show_overlays(&app.overlays);
                        let _ = render(app);
                    }
                }
                id if id >= IDM_MONITOR_BASE && id < IDM_MONITOR_BASE + 64 => {
                    let idx = (id - IDM_MONITOR_BASE) as u32;
                    app.config.mirror_crosshair = false;
                    app.config.set_monitor = idx;
                    reconcile_overlays(app);
                    clear_overlay_bitmaps(&mut app.overlays);
                    if app.visible {
                        show_overlays(&app.overlays);
                        let _ = render(app);
                    }
                }
                IDM_EXIT => {
                    drop_tray_icon(hwnd);
                    let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
                    hide_overlays(&app.overlays);
                    if let Some(h) = app.custom_icon.take() {
                        DestroyIcon(h);
                    }
                    PostQuitMessage(0);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            let app = app_mut(hwnd);
            reconcile_overlays(app);
            if app.visible {
                let _ = render(app);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    } }
}

unsafe extern "system" fn overlay_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if msg == WM_CLOSE_MSG {
            let app = app_mut(hwnd);
            app.visible = false;
            hide_overlays(&app.overlays);
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

// ── Rendering ────────────────────────────────────────────────

unsafe fn render(app: &mut App) -> Result<()> {
    let cfg = app.config.clone();
    let ctype = app.crosshair_type;
    let using_png = cfg.png_crosshair.as_ref().map_or(false, |p| !p.is_empty());
    let (rr, gg, bb) = cfg.parse_color();
    let main_color = (rr, gg, bb, 1.0);
    let (br, bg, bb2) = cfg.parse_border_color();
    let border_color = (br, bg, bb2, cfg.opacity);
    let (mut dpi_x, mut dpi_y) = (96.0f32, 96.0f32);
    unsafe { app.factory.GetDesktopDpi(&mut dpi_x, &mut dpi_y) };
    let scale = dpi_x / 96.0;

    let count = app.overlays.len();
    for i in 0..count {
        let ov = &mut app.overlays[i];
        render_overlay(
            ov, &app.factory, &cfg, ctype, using_png, main_color, border_color, scale,
        )?;
    }
    Ok(())
}

unsafe fn render_overlay(
    ov: &mut OverlayWindow,
    factory: &ID2D1Factory,
    cfg: &Config,
    ctype: CrosshairType,
    using_png: bool,
    main_color: crate::canvas::Color,
    border_color: crate::canvas::Color,
    scale: f32,
) -> Result<()> {
    let hwnd = ov.hwnd;
    let mut rect = unsafe { RECT::default() };
    unsafe { GetClientRect(hwnd, &mut rect) };
    let w = (rect.right - rect.left) as u32;
    let h = (rect.bottom - rect.top) as u32;
    if w == 0 || h == 0 {
        return Ok(());
    }

    let needs_new = match &ov.rt {
        Some(r) => r.width != w || r.height != h,
        None => true,
    };
    if needs_new {
        if let Some(old) = ov.rt.take() {
            if old.width > 0 {
                unsafe { SelectObject(old.mem_dc, HGDIOBJ(old.dib.0 as *mut core::ffi::c_void)) };
            }
            unsafe { DeleteObject(HGDIOBJ(old.dib.0 as *mut core::ffi::c_void)) };
            unsafe { DeleteDC(old.mem_dc) };
        }
        ov.rt = Some(create_rt(w, h, factory)?);
    }

    let res = ov.rt.as_ref().unwrap();
    let target = &res.target;

    use std::ops::Deref;
    let rt: &ID2D1RenderTarget = target.deref();

    unsafe { rt.BeginDraw() };
    unsafe {
        rt.Clear(Some(&D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        } as *const D2D1_COLOR_F));
    }

    let cx = w as f32 / 2.0 + cfg.adjust_x;
    let cy = h as f32 / 2.0 + cfg.adjust_y;

    if using_png {
        if ov.png_bitmap.is_none() || needs_new {
            ov.png_bitmap = png_load_bitmap(cfg, factory, target);
        }
        if let Some(ref bmp) = ov.png_bitmap {
            let bmp_size = unsafe { bmp.GetSize() };
            let img_w = bmp_size.width as f32;
            let img_h = bmp_size.height as f32;
            let target_size = cfg.size * scale;
            let aspect = img_h / img_w;
            let draw_w = target_size;
            let draw_h = target_size * aspect;

            let rotation = cfg.rotation.to_radians();
            if rotation != 0.0 {
                let (sa, ca) = rotation.sin_cos();
                let m = Matrix3x2 {
                    M11: ca, M12: sa,
                    M21: -sa, M22: ca,
                    M31: cx - cx * ca + cy * sa,
                    M32: cy - cx * sa - cy * ca,
                };
                unsafe { target.SetTransform(&m as *const Matrix3x2); }
            }

            let dest = D2D_RECT_F {
                left: cx - draw_w / 2.0,
                top: cy - draw_h / 2.0,
                right: cx + draw_w / 2.0,
                bottom: cy + draw_h / 2.0,
            };
            unsafe {
                let _ = rt.DrawBitmap(
                    bmp,
                    Some(&dest as *const D2D_RECT_F),
                    cfg.opacity,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
            }

            if rotation != 0.0 {
                let identity = Matrix3x2 {
                    M11: 1.0, M12: 0.0,
                    M21: 0.0, M22: 1.0,
                    M31: 0.0, M32: 0.0,
                };
                unsafe { target.SetTransform(&identity as *const Matrix3x2); }
            }
        }
    } else {
        let mut d2d_canvas = D2DCanvas::new(target, w, h);
        crate::crosshair::draw(
            &mut d2d_canvas, main_color, Some(border_color), ctype, cx, cy,
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

    unsafe { rt.EndDraw(None, None)? };

    let mut wr = unsafe { RECT::default() };
    unsafe { GetWindowRect(hwnd, &mut wr) };
    let pt_dst = POINT {
        x: wr.left,
        y: wr.top,
    };
    let size = SIZE {
        cx: w as i32,
        cy: h as i32,
    };
    let pt_src = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: 0,
        BlendFlags: 0,
        SourceConstantAlpha: (cfg.opacity * 255.0) as u8,
        AlphaFormat: 1,
    };

    unsafe {
        UpdateLayeredWindow(
            hwnd,
            None,
            Some(&pt_dst as *const POINT),
            Some(&size as *const SIZE),
            res.mem_dc,
            Some(&pt_src as *const POINT),
            COLORREF::default(),
            Some(&blend as *const BLENDFUNCTION),
            UPDATE_LAYERED_WINDOW_FLAGS(2),
        )
    }
}

unsafe fn create_rt(w: u32, h: u32, factory: &ID2D1Factory) -> Result<RenderResources> { unsafe {
    let mem_dc = unsafe { CreateCompatibleDC(None) };

    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [RGBQUAD::default(); 1],
    };
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    let dib = unsafe { CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) }?;

    unsafe { SelectObject(mem_dc, HGDIOBJ(dib.0 as *mut c_void)) };

    let props = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 0.0,
        dpiY: 0.0,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    };

    let target = factory.CreateDCRenderTarget(&props as *const D2D1_RENDER_TARGET_PROPERTIES)?;

    let bind_rect = RECT {
        left: 0,
        top: 0,
        right: w as i32,
        bottom: h as i32,
    };
    target.BindDC(mem_dc, &bind_rect as *const RECT)?;

    Ok(RenderResources {
        target,
        mem_dc,
        dib,
        width: w,
        height: h,
    })
}}
