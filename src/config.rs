use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CrosshairType {
    Dot,
    Cross,
    T,
    Circle,
    Diamond,
    Arrow,
}

impl CrosshairType {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "dot" => Self::Dot,
            "t" => Self::T,
            "circle" => Self::Circle,
            "diamond" => Self::Diamond,
            "arrow" => Self::Arrow,
            _ => Self::Cross,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dot => "dot",
            Self::Cross => "cross",
            Self::T => "t",
            Self::Circle => "circle",
            Self::Diamond => "diamond",
            Self::Arrow => "arrow",
        }
    }
}

#[derive(Clone)]
pub struct Hotkey {
    pub modifiers: u32,
    pub vk: u16,
}

impl Hotkey {
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').collect();
        if parts.is_empty() || parts[0].trim().is_empty() {
            return None;
        }
        let mut modifiers: u32 = 0;
        for part in &parts[..parts.len() - 1] {
            match part.trim().to_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= 0x0002,
                "shift" => modifiers |= 0x0004,
                "alt" => modifiers |= 0x0001,
                "win" | "super" => modifiers |= 0x0008,
                _ => return None,
            }
        }
        let key_part = parts.last()?.trim();
        let vk = Self::parse_key(key_part)?;
        Some(Self { modifiers, vk })
    }

    pub fn from_parts(primary: &str, secondary: &str) -> Option<Self> {
        let primary = primary.trim();
        let secondary = secondary.trim();
        if primary.is_empty() || secondary.is_empty() {
            return None;
        }
        let mut modifiers: u32 = 0;
        for part in primary.split('+') {
            match part.trim().to_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= 0x0002,
                "shift" => modifiers |= 0x0004,
                "alt" => modifiers |= 0x0001,
                "win" | "super" => modifiers |= 0x0008,
                _ => return None,
            }
        }
        let vk = Self::parse_key(secondary)?;
        Some(Self { modifiers, vk })
    }

    fn parse_key(s: &str) -> Option<u16> {
        match s.to_uppercase().as_str() {
            k if k.len() == 1 && k.as_bytes()[0].is_ascii_alphabetic() => {
                Some(k.as_bytes()[0] as u16)
            }
            k if k.len() == 1 && k.as_bytes()[0].is_ascii_digit() => {
                Some(0x30 + (k.as_bytes()[0] - b'0') as u16)
            }
            k if k.starts_with('F') => {
                let num: u16 = k[1..].parse().ok()?;
                if (1..=24).contains(&num) { Some(0x70 + num - 1) } else { None }
            }
            " " | "SPACE" => Some(0x20),
            "ENTER" | "RETURN" => Some(0x0D),
            "TAB" => Some(0x09),
            "ESC" | "ESCAPE" => Some(0x1B),
            "BACK" | "BACKSPACE" => Some(0x08),
            "DELETE" | "DEL" => Some(0x2E),
            "INSERT" | "INS" => Some(0x2D),
            "HOME" => Some(0x24),
            "END" => Some(0x23),
            "PAGEUP" | "PGUP" => Some(0x21),
            "PAGEDOWN" | "PGDN" => Some(0x22),
            "LEFT" => Some(0x25),
            "RIGHT" => Some(0x27),
            "UP" => Some(0x26),
            "DOWN" => Some(0x28),
            "\\" | "BACKSLASH" => Some(0xDC),
            "-" | "MINUS" => Some(0xBD),
            "=" | "EQUALS" | "PLUS" => Some(0xBB),
            "[" | "LBRACKET" => Some(0xDB),
            "]" | "RBRACKET" => Some(0xDD),
            ";" | "SEMICOLON" => Some(0xBA),
            "'" | "QUOTE" | "APOSTROPHE" => Some(0xDE),
            "," | "COMMA" => Some(0xBC),
            "." | "PERIOD" | "DOT" => Some(0xBE),
            "/" | "SLASH" => Some(0xBF),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub crosshair_type: CrosshairType,
    pub size: f32,
    pub thickness: f32,
    pub thickness_h: f32,
    pub thickness_v: f32,
    pub color_hex: String,
    pub border_color_hex: String,
    pub dot_center: bool,
    pub opacity: f32,
    pub border: bool,
    pub border_size: f32,
    pub space_width: f32,
    pub dot_size: f32,
    pub primary_key: String,
    pub secondary_key: String,
    pub rotation: f32,
    pub png_crosshair: Option<String>,
    pub mirror_crosshair: bool,
    pub set_monitor: u32,
    pub adjust_x: f32,
    pub adjust_y: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            crosshair_type: CrosshairType::Cross,
            size: 24.0,
            thickness: 2.0,
            thickness_h: 2.0,
            thickness_v: 2.0,
            color_hex: "#FF0000".into(),
            border_color_hex: "#000000".into(),
            dot_center: true,
            opacity: 0.85,
            border: true,
            border_size: 0.0,
            space_width: 0.0,
            dot_size: 1.5,
            primary_key: "CTRL".into(),
            secondary_key: "\\".into(),
            rotation: 0.0,
            png_crosshair: None,
            mirror_crosshair: false,
            set_monitor: 0,
            adjust_x: 0.0,
            adjust_y: 0.0,
        }
    }
}

pub fn log_warning(msg: &str) {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let log_path = exe_dir.join("ZeroIn.log");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(f, "{msg}");
    }
}

impl Config {
    pub fn load() -> Self {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        let config_path = exe_dir.join("config.ini");
        let content = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(_) => {
                log_warning("config.ini not found, using defaults");
                return Self::default();
            }
        };

        let mut config = Self::default();
        let mut thickness_explicit = false;
        let mut thickness_h_explicit = false;
        let mut thickness_v_explicit = false;
        let mut current_section = String::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') {
                let end = line.find(']').unwrap_or(line.len());
                current_section = line[1..end].trim().to_lowercase();
                continue;
            }

            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_lowercase();
                let raw = line[eq_pos + 1..].trim();
                let value = raw.split(';').next().unwrap_or(raw).trim();

                if current_section == "crosshair" {
                    match key.as_str() {
                        "type" => config.crosshair_type = CrosshairType::from_str(value),
                        "size" => {
                            let v = value.parse::<f32>().unwrap_or(24.0);
                            if v < 4.0 {
                                log_warning(&format!("size {v} too small, clamped to 4"));
                            }
                            config.size = v.max(4.0);
                        }
                        "thickness" => {
                            let v = value.parse::<f32>().unwrap_or(2.0);
                            if v < 1.0 {
                                log_warning(&format!("thickness {v} too small, clamped to 1"));
                            }
                            config.thickness = v.max(1.0);
                            thickness_explicit = true;
                        }
                        "thickness_h" => {
                            let v = value.parse::<f32>().unwrap_or(2.0);
                            if v < 1.0 {
                                log_warning(&format!("thickness_h {v} too small, clamped to 1"));
                            }
                            config.thickness_h = v.max(1.0);
                            thickness_h_explicit = true;
                        }
                        "thickness_v" => {
                            let v = value.parse::<f32>().unwrap_or(2.0);
                            if v < 1.0 {
                                log_warning(&format!("thickness_v {v} too small, clamped to 1"));
                            }
                            config.thickness_v = v.max(1.0);
                            thickness_v_explicit = true;
                        }
                        "color" => {
                            if !value.starts_with('#') || value.len() < 7 {
                                log_warning(&format!("invalid color {value}, using default"));
                            }
                            config.color_hex = value.to_string();
                        }
                        "dot_center" => {
                            config.dot_center = value.eq_ignore_ascii_case("true")
                                || value == "1"
                        }
                        "opacity" => {
                            let v = value.parse::<f32>().unwrap_or(0.85);
                            if !(0.0..=1.0).contains(&v) {
                                log_warning(&format!("opacity {v} out of range, clamped to 0..1"));
                            }
                            config.opacity = v.clamp(0.0, 1.0);
                        }
                        "border" => {
                            config.border = value.eq_ignore_ascii_case("true")
                                || value == "1"
                        }
                        "border_size" => {
                            let v = value.parse::<f32>().unwrap_or(0.0);
                            if v < 0.0 {
                                log_warning("border_size negative, clamped to 0");
                            }
                            config.border_size = v.max(0.0);
                        }
                        "border_color" => {
                            if !value.starts_with('#') || value.len() < 7 {
                                log_warning(&format!("invalid border_color {value}, using default"));
                            }
                            config.border_color_hex = value.to_string();
                        }
                        "space_width" => {
                            let v = value.parse::<f32>().unwrap_or(0.0).max(0.0);
                            if v < 0.0 {
                                log_warning("space_width negative, clamped to 0");
                            }
                            config.space_width = v;
                        }
                        "dot_size" => {
                            let v = value.parse::<f32>().unwrap_or(1.5);
                            if v < 0.5 || v > 50.0 {
                                log_warning(&format!("dot_size {v} out of range, clamped to 0.5..50"));
                            }
                            config.dot_size = v.clamp(0.5, 50.0);
                        }
                        "png_crosshair" => {
                            let v = value.trim();
                            if v.is_empty() || v.eq_ignore_ascii_case("none") || v.eq_ignore_ascii_case("off") || v.eq_ignore_ascii_case("false") {
                                config.png_crosshair = None;
                            } else {
                                config.png_crosshair = Some(v.to_string());
                            }
                        }
                        "primary_key" => {
                            config.primary_key = value.to_string();
                        }
                        "secondary_key" => {
                            config.secondary_key = value.to_string();
                        }
                        "rotation" => {
                            config.rotation = value.parse::<f32>().unwrap_or(0.0);
                        }
                        "mirror_crosshair" => {
                            config.mirror_crosshair = value.eq_ignore_ascii_case("true")
                                || value == "1"
                        }
                        "set_monitor" => {
                            config.set_monitor = value.parse::<u32>().unwrap_or(0);
                        }
                        "adjust_x" => {
                            config.adjust_x = value.parse::<f32>().unwrap_or(0.0);
                        }
                        "adjust_y" => {
                            config.adjust_y = value.parse::<f32>().unwrap_or(0.0);
                        }
                        _ => {}
                    }
                }
            }
        }

        if !thickness_h_explicit && thickness_explicit {
            config.thickness_h = config.thickness;
        }
        if !thickness_v_explicit && thickness_explicit {
            config.thickness_v = config.thickness;
        }

        if Hotkey::from_parts(&config.primary_key, &config.secondary_key).is_none() {
            log_warning(&format!(
                "invalid hotkey primary={} secondary={}",
                config.primary_key, config.secondary_key
            ));
        }

        config
    }

    pub fn parse_color(&self) -> (f32, f32, f32) {
        let hex = self.color_hex.trim_start_matches('#');
        if hex.len() >= 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
            }
        }
        (1.0, 0.0, 0.0)
    }

    pub fn parse_border_color(&self) -> (f32, f32, f32) {
        let hex = self.border_color_hex.trim_start_matches('#');
        if hex.len() >= 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
            }
        }
        (0.0, 0.0, 0.0)
    }
}
