use core::f32::consts::FRAC_PI_4;

use crate::canvas::{Canvas, Color};
use crate::config::CrosshairType;

pub fn draw(
    canvas: &mut dyn Canvas,
    color: Color,
    border_color: Option<Color>,
    ctype: CrosshairType,
    cx: f32,
    cy: f32,
    size: f32,
    thickness_h: f32,
    thickness_v: f32,
    dot_center: bool,
    border: bool,
    border_size: f32,
    space_width: f32,
    rotation: f32,
    dot_size: f32,
) {
    let intrinsic = match ctype {
        CrosshairType::Diamond => FRAC_PI_4,
        _ => 0.0,
    };
    let total = rotation.to_radians() + intrinsic;

    let has_rotation = total != 0.0;
    if has_rotation {
        canvas.begin_rotation(total, cx, cy);
    }

    match ctype {
        CrosshairType::Dot => draw_dot(canvas, color, border_color, cx, cy, size, border, border_size),
        CrosshairType::Cross | CrosshairType::Diamond => {
            draw_cross(canvas, color, border_color, cx, cy, size, thickness_h, thickness_v, dot_center, border, border_size, space_width, dot_size)
        }
        CrosshairType::T => draw_t(canvas, color, border_color, cx, cy, size, thickness_h, thickness_v, dot_center, border, border_size, space_width, dot_size),
        CrosshairType::Circle => draw_circle(canvas, color, border_color, cx, cy, size, thickness_h, thickness_v, dot_center, border, border_size, space_width, dot_size),
        CrosshairType::Arrow => draw_arrow(canvas, color, border_color, cx, cy, size, thickness_h, thickness_v, dot_center, border, border_size, space_width, dot_size),
    }

    if has_rotation {
        canvas.end_rotation();
    }
}

fn draw_split_rect(
    canvas: &mut dyn Canvas,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    color: Color,
) {
    if left < right && top < bottom {
        canvas.fill_rect(left, top, right, bottom, color);
    }
}

fn draw_dot(
    canvas: &mut dyn Canvas,
    color: Color,
    border_color: Option<Color>,
    cx: f32,
    cy: f32,
    size: f32,
    border: bool,
    border_size: f32,
) {
    let radius = size / 2.0;

    if border && border_size > 0.0 {
        if let Some(bc) = border_color {
            canvas.fill_ellipse(cx, cy, radius + border_size, radius + border_size, bc);
        }
    }

    canvas.fill_ellipse(cx, cy, radius, radius, color);
}

fn draw_cross(
    canvas: &mut dyn Canvas,
    color: Color,
    border_color: Option<Color>,
    cx: f32,
    cy: f32,
    size: f32,
    thickness_h: f32,
    thickness_v: f32,
    dot_center: bool,
    border: bool,
    border_size: f32,
    space_width: f32,
    dot_size: f32,
) {
    let half = size / 2.0;
    let half_t_h = thickness_h / 2.0;
    let half_t_v = thickness_v / 2.0;
    let sw = space_width.min(half);
    let bs = border_size;

    if border && bs > 0.0 {
        if let Some(bc) = border_color {
            draw_split_rect(canvas, cx - half - bs, cy - half_t_v - bs, cx - sw + bs, cy + half_t_v + bs, bc);
            draw_split_rect(canvas, cx + sw - bs, cy - half_t_v - bs, cx + half + bs, cy + half_t_v + bs, bc);
            draw_split_rect(canvas, cx - half_t_h - bs, cy - half - bs, cx + half_t_h + bs, cy - sw + bs, bc);
            draw_split_rect(canvas, cx - half_t_h - bs, cy + sw - bs, cx + half_t_h + bs, cy + half + bs, bc);
            if dot_center {
                canvas.fill_ellipse(cx, cy, dot_size + bs, dot_size + bs, bc);
            }
        }
    }

    draw_split_rect(canvas, cx - half, cy - half_t_v, cx - sw, cy + half_t_v, color);
    draw_split_rect(canvas, cx + sw, cy - half_t_v, cx + half, cy + half_t_v, color);
    draw_split_rect(canvas, cx - half_t_h, cy - half, cx + half_t_h, cy - sw, color);
    draw_split_rect(canvas, cx - half_t_h, cy + sw, cx + half_t_h, cy + half, color);

    if dot_center {
        canvas.fill_ellipse(cx, cy, dot_size, dot_size, color);
    }
}

fn draw_t(
    canvas: &mut dyn Canvas,
    color: Color,
    border_color: Option<Color>,
    cx: f32,
    cy: f32,
    size: f32,
    thickness_h: f32,
    thickness_v: f32,
    dot_center: bool,
    border: bool,
    border_size: f32,
    space_width: f32,
    dot_size: f32,
) {
    let half = size / 2.0;
    let half_t_h = thickness_h / 2.0;
    let half_t_v = thickness_v / 2.0;
    let sw = space_width.min(half);
    let bs = border_size;

    if border && bs > 0.0 {
        if let Some(bc) = border_color {
            draw_split_rect(canvas, cx - half - bs, cy - half_t_v - bs, cx - sw + bs, cy + half_t_v + bs, bc);
            draw_split_rect(canvas, cx + sw - bs, cy - half_t_v - bs, cx + half + bs, cy + half_t_v + bs, bc);
            draw_split_rect(canvas, cx - half_t_h - bs, cy - half_t_h - bs, cx + half_t_h + bs, cy - sw + bs, bc);
            draw_split_rect(canvas, cx - half_t_h - bs, cy + sw - bs, cx + half_t_h + bs, cy + half + bs, bc);
            if dot_center {
                canvas.fill_ellipse(cx, cy, dot_size + bs, dot_size + bs, bc);
            }
        }
    }

    draw_split_rect(canvas, cx - half, cy - half_t_v, cx - sw, cy + half_t_v, color);
    draw_split_rect(canvas, cx + sw, cy - half_t_v, cx + half, cy + half_t_v, color);
    draw_split_rect(canvas, cx - half_t_h, cy - half_t_h, cx + half_t_h, cy - sw, color);
    draw_split_rect(canvas, cx - half_t_h, cy + sw, cx + half_t_h, cy + half, color);

    if dot_center {
        canvas.fill_ellipse(cx, cy, dot_size, dot_size, color);
    }
}

fn draw_circle(
    canvas: &mut dyn Canvas,
    color: Color,
    border_color: Option<Color>,
    cx: f32,
    cy: f32,
    size: f32,
    thickness_h: f32,
    _thickness_v: f32,
    dot_center: bool,
    border: bool,
    border_size: f32,
    space_width: f32,
    dot_size: f32,
) {
    let outer_r = size / 2.0;
    let stroke = thickness_h;
    let bs = border_size;
    let outline_mode = border;

    if bs > 0.0 {
        if let Some(bc) = border_color {
            if outline_mode {
                canvas.draw_ellipse(cx, cy, outer_r + bs, outer_r + bs, stroke + bs * 2.0, bc);
            } else if dot_center {
                canvas.fill_ellipse(cx, cy, dot_size + bs, dot_size + bs, bc);
            } else {
                canvas.fill_ellipse(cx, cy, outer_r + bs, outer_r + bs, bc);
            }
            if dot_center {
                canvas.fill_ellipse(cx, cy, dot_size + bs, dot_size + bs, bc);
            }
        }
    }

    if outline_mode && dot_center {
        let max_dot = (outer_r - stroke / 2.0 - space_width).max(0.0);
        let actual_dot_r = dot_size.min(max_dot);
        canvas.draw_ellipse(cx, cy, outer_r, outer_r, stroke, color);
        if actual_dot_r > 0.0 {
            canvas.fill_ellipse(cx, cy, actual_dot_r, actual_dot_r, color);
        }
    } else if outline_mode && !dot_center {
        canvas.draw_ellipse(cx, cy, outer_r, outer_r, stroke, color);
    } else if !outline_mode && dot_center {
        canvas.fill_ellipse(cx, cy, dot_size, dot_size, color);
    } else {
        canvas.fill_ellipse(cx, cy, outer_r, outer_r, color);
    }
}

fn draw_arrow(
    canvas: &mut dyn Canvas,
    color: Color,
    border_color: Option<Color>,
    cx: f32,
    cy: f32,
    size: f32,
    thickness_h: f32,
    thickness_v: f32,
    dot_center: bool,
    border: bool,
    border_size: f32,
    space_width: f32,
    dot_size: f32,
) {
    let half = size / 2.0;
    let sw = space_width.min(half);
    let arm = (half - sw) * 0.55;
    let half_t_h = thickness_h / 2.0;
    let half_t_v = thickness_v / 2.0;
    let bs = border_size;

    let lines_h = [
        (cx + sw, cy - half_t_v, cx + sw + arm, cy - arm),
        (cx + sw, cy + half_t_v, cx + sw + arm, cy + arm),
        (cx - sw, cy - half_t_v, cx - sw - arm, cy - arm),
        (cx - sw, cy + half_t_v, cx - sw - arm, cy + arm),
    ];
    let lines_v = [
        (cx - half_t_h, cy - sw, cx - arm, cy - sw - arm),
        (cx + half_t_h, cy - sw, cx + arm, cy - sw - arm),
        (cx - half_t_h, cy + sw, cx - arm, cy + sw + arm),
        (cx + half_t_h, cy + sw, cx + arm, cy + sw + arm),
    ];

    if border && bs > 0.0 {
        if let Some(bc) = border_color {
            let th = thickness_v + bs * 2.0;
            let tv = thickness_h + bs * 2.0;
            for &(x1, y1, x2, y2) in &lines_h {
                canvas.draw_line(x1, y1, x2, y2, th, bc);
            }
            for &(x1, y1, x2, y2) in &lines_v {
                canvas.draw_line(x1, y1, x2, y2, tv, bc);
            }
            if dot_center {
                canvas.fill_ellipse(cx, cy, dot_size + bs, dot_size + bs, bc);
            }
        }
    }

    for &(x1, y1, x2, y2) in &lines_h {
        canvas.draw_line(x1, y1, x2, y2, thickness_v, color);
    }
    for &(x1, y1, x2, y2) in &lines_v {
        canvas.draw_line(x1, y1, x2, y2, thickness_h, color);
    }

    if dot_center {
        canvas.fill_ellipse(cx, cy, dot_size, dot_size, color);
    }
}
