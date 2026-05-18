#![windows_subsystem = "windows"]

mod config;
mod crosshair;

use core::ffi::c_void;
use std::mem;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
    D2D1_RENDER_TARGET_USAGE_NONE, ID2D1DCRenderTarget, ID2D1Factory, ID2D1RenderTarget,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT, SetProcessDpiAwarenessContext};
use windows::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use config::{Config, CrosshairType, Hotkey};

const WM_TRAYICON: u32 = WM_APP + 1;
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
const IDM_RELOAD_CONFIG: u16 = 1007;
const IDM_EXIT: u16 = 1005;

struct App {
    overlay_hwnd: HWND,
    factory: ID2D1Factory,
    rt: Option<RenderResources>,
    config: Config,
    crosshair_type: CrosshairType,
    visible: bool,
    custom_icon: Option<HICON>,
}

struct RenderResources {
    target: ID2D1DCRenderTarget,
    mem_dc: HDC,
    dib: HBITMAP,
    width: u32,
    height: u32,
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
            GDI_IMAGE_TYPE(1),   // IMAGE_ICON
            0,                   // default width
            0,                   // default height
            IMAGE_FLAGS(0x10),   // LR_LOADFROMFILE
        ) {
            Ok(h) if h != HANDLE::default() => Some(mem::transmute(h)),
            _ => None,
        }
    }
}

// ── Wide string helper ──────────────────────────────────────

struct WStr(Vec<u16>);
impl WStr {
    fn new(s: &str) -> Self {
        Self(s.encode_utf16().chain(Some(0)).collect())
    }
    fn as_pcwstr(&self) -> PCWSTR {
        PCWSTR(self.0.as_ptr())
    }
}

// ── Entry ──────────────────────────────────────────────────

fn main() -> Result<()> {
    unsafe {
        #[allow(clippy::cast_possible_wrap)]
        let ctx = DPI_AWARENESS_CONTEXT((-4isize) as *mut c_void);
        if SetProcessDpiAwarenessContext(ctx).is_err() {
            let _ = SetProcessDPIAware();
        }
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok();

        let hmodule = GetModuleHandleW(None)?;
        let hinstance: HINSTANCE = mem::transmute(hmodule);
        let cfg = Config::load();

        let custom_icon = load_custom_icon();
        let default_icon = LoadIconW(None, IDI_APPLICATION)?;
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

        let mon_rect = primary_monitor_rect();
        let ow = mon_rect.right - mon_rect.left;
        let oh = mon_rect.bottom - mon_rect.top;

        let overlay_hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            oc.as_pcwstr(),
            WStr::new("").as_pcwstr(),
            WS_POPUP,
            mon_rect.left, mon_rect.top, ow, oh,
            None, None, hinstance, None,
        )?;

        let factory: ID2D1Factory =
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;

        let app = Box::new(App {
            overlay_hwnd,
            factory,
            rt: None,
            config: cfg.clone(),
            crosshair_type: cfg.crosshair_type,
            visible: false,
            custom_icon,
        });
        let app_ptr = Box::into_raw(app);

        SetWindowLongPtrW(tray_hwnd, GWLP_USERDATA, app_ptr as isize);
        SetWindowLongPtrW(overlay_hwnd, GWLP_USERDATA, app_ptr as isize);

        create_tray_icon(tray_hwnd, icon)?;

        if let Some(hk) = Hotkey::from_parts(&cfg.primary_key, &cfg.secondary_key) {
            let _ = RegisterHotKey(
                tray_hwnd,
                HOTKEY_ID,
                HOT_KEY_MODIFIERS(hk.modifiers),
                hk.vk as u32,
            );
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        drop_tray_icon(tray_hwnd);
        let _ = Box::from_raw(app_ptr);
        CoUninitialize();
        Ok(())
    }
}

// ── Helpers ─────────────────────────────────────────────────

unsafe fn app_mut(hwnd: HWND) -> &'static mut App {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App };
    unsafe { &mut *ptr }
}

unsafe fn primary_monitor_rect() -> RECT {
    let pt = POINT { x: 0, y: 0 };
    let hm = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTOPRIMARY) };
    let mut mi = MONITORINFO {
        cbSize: mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetMonitorInfoW(hm, &mut mi) };
    mi.rcMonitor
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
    let tip = WStr::new("Crosshair");
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
    let hsub = unsafe { CreatePopupMenu() }.unwrap();

    unsafe {
        AppendMenuW(hsub, MENU_ITEM_FLAGS(0), IDM_TYPE_DOT as usize, WStr::new("Dot").as_pcwstr());
        AppendMenuW(hsub, MENU_ITEM_FLAGS(0), IDM_TYPE_CROSS as usize, WStr::new("Cross").as_pcwstr());
        AppendMenuW(hsub, MENU_ITEM_FLAGS(0), IDM_TYPE_T as usize, WStr::new("T").as_pcwstr());
        AppendMenuW(hsub, MENU_ITEM_FLAGS(0), IDM_TYPE_CIRCLE as usize, WStr::new("Circle").as_pcwstr());
        AppendMenuW(hsub, MENU_ITEM_FLAGS(0), IDM_TYPE_DIAMOND as usize, WStr::new("Diamond").as_pcwstr());
        AppendMenuW(hsub, MENU_ITEM_FLAGS(0), IDM_TYPE_ARROW as usize, WStr::new("Arrow").as_pcwstr());
    }

    match app.crosshair_type {
        CrosshairType::Dot => unsafe { CheckMenuItem(hsub, IDM_TYPE_DOT as u32, MF_CHECKED.0) },
        CrosshairType::Cross => unsafe { CheckMenuItem(hsub, IDM_TYPE_CROSS as u32, MF_CHECKED.0) },
        CrosshairType::T => unsafe { CheckMenuItem(hsub, IDM_TYPE_T as u32, MF_CHECKED.0) },
        CrosshairType::Circle => unsafe { CheckMenuItem(hsub, IDM_TYPE_CIRCLE as u32, MF_CHECKED.0) },
        CrosshairType::Diamond => unsafe { CheckMenuItem(hsub, IDM_TYPE_DIAMOND as u32, MF_CHECKED.0) },
        CrosshairType::Arrow => unsafe { CheckMenuItem(hsub, IDM_TYPE_ARROW as u32, MF_CHECKED.0) },
    };

    let toggle_flags = MENU_ITEM_FLAGS(MF_STRING.0)
        | if app.visible { MF_CHECKED } else { MENU_ITEM_FLAGS(0) };

    unsafe {
        AppendMenuW(hmenu, toggle_flags, IDM_TOGGLE as usize, WStr::new("Toggle On/Off").as_pcwstr());
        AppendMenuW(hmenu, MENU_ITEM_FLAGS(MF_SEPARATOR.0), 0, None);
        AppendMenuW(
            hmenu,
            MENU_ITEM_FLAGS(MF_POPUP.0 | MF_STRING.0),
            mem::transmute::<HMENU, usize>(hsub),
            WStr::new("Crosshair Type").as_pcwstr(),
        );
        AppendMenuW(hmenu, MENU_ITEM_FLAGS(MF_SEPARATOR.0), 0, None);
        AppendMenuW(hmenu, MENU_ITEM_FLAGS(0), IDM_RELOAD_CONFIG as usize, WStr::new("Reload Config").as_pcwstr());
        AppendMenuW(hmenu, MENU_ITEM_FLAGS(MF_SEPARATOR.0), 0, None);
        AppendMenuW(hmenu, MENU_ITEM_FLAGS(0), IDM_EXIT as usize, WStr::new("Exit").as_pcwstr());
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
        WM_HOTKEY_MSG => {
            let app = app_mut(hwnd);
            app.visible = !app.visible;
            if app.visible {
                app.config = Config::load();
                app.crosshair_type = app.config.crosshair_type;
                ShowWindow(app.overlay_hwnd, SW_SHOW);
                SetWindowPos(
                    app.overlay_hwnd,
                    HWND_TOPMOST,
                    0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                );
                let _ = render(app);
            } else {
                ShowWindow(app.overlay_hwnd, SW_HIDE);
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
                        app.config = Config::load();
                        app.crosshair_type = app.config.crosshair_type;
                        ShowWindow(app.overlay_hwnd, SW_SHOW);
                        SetWindowPos(
                            app.overlay_hwnd,
                            HWND_TOPMOST,
                            0, 0, 0, 0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                        );
                        let _ = render(app);
                    } else {
                        ShowWindow(app.overlay_hwnd, SW_HIDE);
                    }
                }
                IDM_TYPE_DOT => {
                    app.crosshair_type = CrosshairType::Dot;
                    if app.visible { let _ = render(app); }
                }
                IDM_TYPE_CROSS => {
                    app.crosshair_type = CrosshairType::Cross;
                    if app.visible { let _ = render(app); }
                }
                IDM_TYPE_T => {
                    app.crosshair_type = CrosshairType::T;
                    if app.visible { let _ = render(app); }
                }
                IDM_TYPE_CIRCLE => {
                    app.crosshair_type = CrosshairType::Circle;
                    if app.visible { let _ = render(app); }
                }
                IDM_TYPE_DIAMOND => {
                    app.crosshair_type = CrosshairType::Diamond;
                    if app.visible { let _ = render(app); }
                }
                IDM_TYPE_ARROW => {
                    app.crosshair_type = CrosshairType::Arrow;
                    if app.visible { let _ = render(app); }
                }
                IDM_RELOAD_CONFIG => {
                    app.config = Config::load();
                    app.crosshair_type = app.config.crosshair_type;
                    if app.visible {
                        let _ = render(app);
                    }
                }
                IDM_EXIT => {
                    drop_tray_icon(hwnd);
                    let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
                    ShowWindow(app.overlay_hwnd, SW_HIDE);
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
            let mon_rect = primary_monitor_rect();
            let ow = mon_rect.right - mon_rect.left;
            let oh = mon_rect.bottom - mon_rect.top;
            SetWindowPos(
                app.overlay_hwnd,
                HWND_TOPMOST,
                mon_rect.left, mon_rect.top, ow, oh,
                SWP_NOZORDER,
            );
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
            ShowWindow(hwnd, SW_HIDE);
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

// Keep the outer unsafe and wrap all unsafe calls in explicit unsafe blocks

// ── Rendering ────────────────────────────────────────────────

unsafe fn render(app: &mut App) -> Result<()> { unsafe {
    let hwnd = app.overlay_hwnd;
    let mut rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rect) };
    let w = (rect.right - rect.left) as u32;
    let h = (rect.bottom - rect.top) as u32;
    if w == 0 || h == 0 {
        return Ok(());
    }

    let needs_new = match &app.rt {
        Some(r) => r.width != w || r.height != h,
        None => true,
    };
    if needs_new {
        if let Some(old) = app.rt.take() {
            if old.width > 0 {
                unsafe { SelectObject(old.mem_dc, HGDIOBJ(old.dib.0 as *mut c_void)) };
            }
            unsafe { DeleteObject(HGDIOBJ(old.dib.0 as *mut c_void)) };
            unsafe { DeleteDC(old.mem_dc) };
        }
        app.rt = Some(create_rt(w, h, &app.factory)?);
    }

    let res = app.rt.as_ref().unwrap();
    let target = &res.target;

    use std::ops::Deref;
    let rt: &ID2D1RenderTarget = target.deref();

    rt.BeginDraw();
    rt.Clear(Some(&D2D1_COLOR_F {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    } as *const D2D1_COLOR_F));

    let (rr, gg, bb) = app.config.parse_color();
    let color = D2D1_COLOR_F {
        r: rr,
        g: gg,
        b: bb,
        a: 1.0,
    };
    let brush = rt.CreateSolidColorBrush(
        &color as *const D2D1_COLOR_F,
        None,
    )?;

    let (mut dpi_x, mut dpi_y) = (96.0f32, 96.0f32);
    app.factory.GetDesktopDpi(&mut dpi_x, &mut dpi_y);
    let scale = dpi_x / 96.0;
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    crosshair::draw(
        target, &brush, app.crosshair_type, cx, cy,
        app.config.size * scale, app.config.thickness * scale,
        app.config.dot_center,
        app.config.border, app.config.space_width * scale,
        app.config.rotation,
    );

    rt.EndDraw(None, None)?;

    let mut wr = RECT::default();
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
        SourceConstantAlpha: (app.config.opacity * 255.0) as u8,
        AlphaFormat: 1, // AC_SRC_ALPHA
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
            UPDATE_LAYERED_WINDOW_FLAGS(2), // ULW_ALPHA
        )
    }
}}

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
