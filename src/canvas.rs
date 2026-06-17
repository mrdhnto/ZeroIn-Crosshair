pub type Color = (f32, f32, f32, f32);

pub trait Canvas {
    fn fill_rect(&mut self, left: f32, top: f32, right: f32, bottom: f32, color: Color);
    fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, color: Color);
    fn draw_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, stroke: f32, color: Color);
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: Color);
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn begin_rotation(&mut self, angle_rad: f32, cx: f32, cy: f32);
    fn end_rotation(&mut self);
}

#[cfg(windows)]
pub struct D2DCanvas<'a> {
    target: &'a windows::Win32::Graphics::Direct2D::ID2D1DCRenderTarget,
    width: u32,
    height: u32,
}

#[cfg(windows)]
impl<'a> D2DCanvas<'a> {
    pub fn new(
        target: &'a windows::Win32::Graphics::Direct2D::ID2D1DCRenderTarget,
        width: u32,
        height: u32,
    ) -> Self {
        Self { target, width, height }
    }
}

#[cfg(windows)]
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_POINT_2F, D2D_RECT_F};
#[cfg(windows)]
use windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE;

#[cfg(windows)]
impl<'a> Canvas for D2DCanvas<'a> {
    fn fill_rect(&mut self, left: f32, top: f32, right: f32, bottom: f32, color: Color) {
        let c = D2D1_COLOR_F { r: color.0, g: color.1, b: color.2, a: color.3 };
        unsafe {
            if let Ok(brush) = self.target.CreateSolidColorBrush(&c as *const D2D1_COLOR_F, None) {
                let _ = self.target.FillRectangle(&D2D_RECT_F { left, top, right, bottom }, &brush);
            }
        }
    }

    fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, color: Color) {
        let c = D2D1_COLOR_F { r: color.0, g: color.1, b: color.2, a: color.3 };
        unsafe {
            if let Ok(brush) = self.target.CreateSolidColorBrush(&c as *const D2D1_COLOR_F, None) {
                let _ = self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: D2D_POINT_2F { x: cx, y: cy },
                        radiusX: rx,
                        radiusY: ry,
                    },
                    &brush,
                );
            }
        }
    }

    fn draw_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, stroke: f32, color: Color) {
        let c = D2D1_COLOR_F { r: color.0, g: color.1, b: color.2, a: color.3 };
        unsafe {
            if let Ok(brush) = self.target.CreateSolidColorBrush(&c as *const D2D1_COLOR_F, None) {
                let _ = self.target.DrawEllipse(
                    &D2D1_ELLIPSE {
                        point: D2D_POINT_2F { x: cx, y: cy },
                        radiusX: rx,
                        radiusY: ry,
                    },
                    &brush,
                    stroke,
                    None,
                );
            }
        }
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: Color) {
        let c = D2D1_COLOR_F { r: color.0, g: color.1, b: color.2, a: color.3 };
        unsafe {
            if let Ok(brush) = self.target.CreateSolidColorBrush(&c as *const D2D1_COLOR_F, None) {
                let _ = self.target.DrawLine(
                    D2D_POINT_2F { x: x1, y: y1 },
                    D2D_POINT_2F { x: x2, y: y2 },
                    &brush,
                    thickness,
                    None,
                );
            }
        }
    }

    fn width(&self) -> u32 { self.width }
    fn height(&self) -> u32 { self.height }

    fn begin_rotation(&mut self, angle_rad: f32, cx: f32, cy: f32) {
        use windows::Foundation::Numerics::Matrix3x2;
        let (sa, ca) = angle_rad.sin_cos();
        let m = Matrix3x2 {
            M11: ca, M12: sa,
            M21: -sa, M22: ca,
            M31: cx - cx * ca + cy * sa,
            M32: cy - cx * sa - cy * ca,
        };
        unsafe { self.target.SetTransform(&m as *const Matrix3x2); }
    }

    fn end_rotation(&mut self) {
        use windows::Foundation::Numerics::Matrix3x2;
        let identity = Matrix3x2 {
            M11: 1.0, M12: 0.0,
            M21: 0.0, M22: 1.0,
            M31: 0.0, M32: 0.0,
        };
        unsafe { self.target.SetTransform(&identity as *const Matrix3x2); }
    }
}

#[cfg(target_os = "linux")]
pub struct SwCanvas {
    pixels: Vec<u32>,
    width: u32,
    height: u32,
    rotate_stack: Vec<RotateSection>,
}

#[cfg(target_os = "linux")]
struct RotateSection {
    buffer: Vec<u32>,
    angle_rad: f32,
    cx: f32,
    cy: f32,
}

#[cfg(target_os = "linux")]
impl SwCanvas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            pixels: vec![0u32; (width * height) as usize],
            width,
            height,
            rotate_stack: Vec::new(),
        }
    }

    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [u32] {
        &mut self.pixels
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    fn current_buffer_mut(&mut self) -> &mut Vec<u32> {
        if let Some(section) = self.rotate_stack.last_mut() {
            &mut section.buffer
        } else {
            &mut self.pixels
        }
    }

    fn current_buffer(&self) -> &[u32] {
        if let Some(section) = self.rotate_stack.last() {
            &section.buffer
        } else {
            &self.pixels
        }
    }
}

#[cfg(target_os = "linux")]
fn premul_pixel(color: Color) -> u32 {
    let (r, g, b, a) = color;
    let a = a.clamp(0.0, 1.0);
    let pb = (b * a * 255.0).round().min(255.0).max(0.0) as u32;
    let pg = (g * a * 255.0).round().min(255.0).max(0.0) as u32;
    let pr = (r * a * 255.0).round().min(255.0).max(0.0) as u32;
    let pa = (a * 255.0).round().min(255.0).max(0.0) as u32;
    (pa << 24) | (pr << 16) | (pg << 8) | pb
}

#[cfg(target_os = "linux")]
pub(crate) fn blend_premul(dst: u32, src: u32) -> u32 {
    if src >> 24 == 0 { return dst; }
    if src >> 24 == 255 { return src; }
    let sa = (src >> 24) as f32 / 255.0;
    let inv_sa = 1.0 - sa;
    let sb = (src & 0xFF) as f32;
    let sg = ((src >> 8) & 0xFF) as f32;
    let sr = ((src >> 16) & 0xFF) as f32;
    let db = (dst & 0xFF) as f32;
    let dg = ((dst >> 8) & 0xFF) as f32;
    let dr = ((dst >> 16) & 0xFF) as f32;
    let da = ((dst >> 24) & 0xFF) as f32;
    let ob = (sb + db * inv_sa).round().min(255.0) as u32;
    let og = (sg + dg * inv_sa).round().min(255.0) as u32;
    let or_ = (sr + dr * inv_sa).round().min(255.0) as u32;
    let oa = (sa * 255.0 + da * inv_sa).round().min(255.0) as u32;
    (oa << 24) | (or_ << 16) | (og << 8) | ob
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
fn rotate_buffer(src: &[u32], w: u32, h: u32, angle_rad: f32, cx: f32, cy: f32) -> Vec<u32> {
    let (sin_a, cos_a) = angle_rad.sin_cos();
    let mut dst = vec![0u32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let sx = dx * cos_a + dy * sin_a + cx;
            let sy = -dx * sin_a + dy * cos_a + cy;
            if sx >= 0.0 && sx < w as f32 - 1.0 && sy >= 0.0 && sy < h as f32 - 1.0 {
                let ix = sx as u32;
                let iy = sy as u32;
                let fx = ((sx - sx.floor()) * 255.0) as u32;
                let fy = ((sy - sy.floor()) * 255.0) as u32;
                let fx_inv = 255 - fx;
                let fy_inv = 255 - fy;
                let p00 = src[(iy * w + ix) as usize];
                let p10 = src[(iy * w + (ix + 1).min(w - 1)) as usize];
                let p01 = src[((iy + 1).min(h - 1) * w + ix) as usize];
                let p11 = src[((iy + 1).min(h - 1) * w + (ix + 1).min(w - 1)) as usize];
                let row0 = lerp_pixel(p00, p10, fx, fx_inv);
                let row1 = lerp_pixel(p01, p11, fx, fx_inv);
                dst[(y * w + x) as usize] = lerp_pixel(row0, row1, fy, fy_inv);
            }
        }
    }
    dst
}

#[cfg(target_os = "linux")]
impl Canvas for SwCanvas {
    fn fill_rect(&mut self, left: f32, top: f32, right: f32, bottom: f32, color: Color) {
        let w = self.width;
        let h = self.height;
        let buf = self.current_buffer_mut();
        let l = left.max(0.0) as u32;
        let t = top.max(0.0) as u32;
        let r = (right.min(w as f32)) as u32;
        let b = (bottom.min(h as f32)) as u32;
        if l >= r || t >= b { return; }
        let pixel = premul_pixel(color);
        if pixel >> 24 == 0 { return; }
        for y in t..b {
            for x in l..r {
                let idx = (y * w + x) as usize;
                buf[idx] = blend_premul(buf[idx], pixel);
            }
        }
    }

    fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, color: Color) {
        if rx <= 0.0 || ry <= 0.0 { return; }
        let w = self.width;
        let h = self.height;
        let buf = self.current_buffer_mut();
        let pixel = premul_pixel(color);
        if pixel >> 24 == 0 { return; }
        let rxx = rx * rx;
        let ryy = ry * ry;
        let xmin = ((cx - rx).max(0.0) as u32).min(w.saturating_sub(1));
        let xmax = ((cx + rx).min((w - 1) as f32)) as u32;
        let ymin = ((cy - ry).max(0.0) as u32).min(h.saturating_sub(1));
        let ymax = ((cy + ry).min((h - 1) as f32)) as u32;
        for y in ymin..=ymax {
            for x in xmin..=xmax {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                if (dx * dx) / rxx + (dy * dy) / ryy <= 1.0 {
                    let idx = (y * w + x) as usize;
                    buf[idx] = blend_premul(buf[idx], pixel);
                }
            }
        }
    }

    fn draw_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, stroke: f32, color: Color) {
        if rx <= 0.0 || ry <= 0.0 || stroke <= 0.0 { return; }
        let half_s = stroke / 2.0;
        let outer_rx = rx + half_s;
        let outer_ry = ry + half_s;
        let inner_rx = (rx - half_s).max(0.0);
        let inner_ry = (ry - half_s).max(0.0);
        let w = self.width;
        let h = self.height;
        let buf = self.current_buffer_mut();
        let pixel = premul_pixel(color);
        if pixel >> 24 == 0 { return; }
        let outer_rxx = outer_rx * outer_rx;
        let outer_ryy = outer_ry * outer_ry;
        let inner_rxx = inner_rx * inner_rx;
        let inner_ryy = inner_ry * inner_ry;
        let xmin = ((cx - outer_rx).max(0.0) as u32).min(w.saturating_sub(1));
        let xmax = ((cx + outer_rx).min((w - 1) as f32)) as u32;
        let ymin = ((cy - outer_ry).max(0.0) as u32).min(h.saturating_sub(1));
        let ymax = ((cy + outer_ry).min((h - 1) as f32)) as u32;
        for y in ymin..=ymax {
            for x in xmin..=xmax {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let d_out = (dx * dx) / outer_rxx + (dy * dy) / outer_ryy;
                if d_out <= 1.0 {
                    if inner_rxx > 0.0 && inner_ryy > 0.0 {
                        let d_in = (dx * dx) / inner_rxx + (dy * dy) / inner_ryy;
                        if d_in > 1.0 {
                            let idx = (y * w + x) as usize;
                            buf[idx] = blend_premul(buf[idx], pixel);
                        }
                    } else {
                        let idx = (y * w + x) as usize;
                        buf[idx] = blend_premul(buf[idx], pixel);
                    }
                }
            }
        }
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: Color) {
        if thickness <= 0.0 { return; }
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len_sq = dx * dx + dy * dy;
        if len_sq < 0.0001 {
            self.fill_ellipse(x1, y1, thickness / 2.0, thickness / 2.0, color);
            return;
        }
        let half = thickness / 2.0;
        let pixel = premul_pixel(color);
        if pixel >> 24 == 0 { return; }
        let w = self.width;
        let h = self.height;
        let buf = self.current_buffer_mut();
        let min_x = ((x1.min(x2) - half).floor().max(0.0) as u32).min(w.saturating_sub(1));
        let max_x = ((x1.max(x2) + half).ceil().min((w - 1) as f32)) as u32;
        let min_y = ((y1.min(y2) - half).floor().max(0.0) as u32).min(h.saturating_sub(1));
        let max_y = ((y1.max(y2) + half).ceil().min((h - 1) as f32)) as u32;
        let inv_len_sq = 1.0 / len_sq;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32;
                let py = y as f32;
                let t = ((px - x1) * dx + (py - y1) * dy) * inv_len_sq;
                let t = t.clamp(0.0, 1.0);
                let near_x = x1 + t * dx;
                let near_y = y1 + t * dy;
                let dist2 = (px - near_x).powi(2) + (py - near_y).powi(2);
                if dist2 <= half * half {
                    let idx = (y * w + x) as usize;
                    buf[idx] = blend_premul(buf[idx], pixel);
                }
            }
        }
    }

    fn width(&self) -> u32 { self.width }
    fn height(&self) -> u32 { self.height }

    fn begin_rotation(&mut self, angle_rad: f32, cx: f32, cy: f32) {
        self.rotate_stack.push(RotateSection {
            buffer: vec![0u32; (self.width * self.height) as usize],
            angle_rad,
            cx,
            cy,
        });
    }

    fn end_rotation(&mut self) {
        if let Some(section) = self.rotate_stack.pop() {
            let rotated = rotate_buffer(&section.buffer, self.width, self.height, section.angle_rad, section.cx, section.cy);
            let target = if let Some(parent) = self.rotate_stack.last_mut() {
                &mut parent.buffer
            } else {
                &mut self.pixels
            };
            for (i, &src_px) in rotated.iter().enumerate() {
                if src_px >> 24 != 0 {
                    target[i] = blend_premul(target[i], src_px);
                }
            }
        }
    }
}
