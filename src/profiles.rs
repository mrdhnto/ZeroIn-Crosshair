use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_exe: Option<String>,
    pub crosshair_type: String,
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
    pub rotation: f32,
    pub dot_size: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub png_crosshair: Option<String>,
    pub mirror_crosshair: bool,
    pub set_monitor: u32,
    pub adjust_x: f32,
    pub adjust_y: f32,
}

#[derive(Debug, Clone)]
pub struct Profiles {
    pub list: Vec<Profile>,
    pub current: Option<usize>,
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

impl Profile {
    pub fn apply_to_config(&self, config: &mut crate::config::Config) {
        config.crosshair_type = crate::config::CrosshairType::from_str(&self.crosshair_type);
        config.size = self.size;
        config.thickness = self.thickness;
        config.thickness_h = self.thickness_h;
        config.thickness_v = self.thickness_v;
        config.color_hex = self.color_hex.clone();
        config.border_color_hex = self.border_color_hex.clone();
        config.dot_center = self.dot_center;
        config.opacity = self.opacity;
        config.border = self.border;
        config.border_size = self.border_size;
        config.space_width = self.space_width;
        config.rotation = self.rotation;
        config.dot_size = self.dot_size;
        config.png_crosshair = self.png_crosshair.clone();
        config.mirror_crosshair = self.mirror_crosshair;
        config.set_monitor = self.set_monitor;
        config.adjust_x = self.adjust_x;
        config.adjust_y = self.adjust_y;
    }

    pub fn from_config(name: String, config: &crate::config::Config) -> Self {
        Self {
            name,
            game_exe: None,
            crosshair_type: config.crosshair_type.as_str().to_string(),
            size: config.size,
            thickness: config.thickness,
            thickness_h: config.thickness_h,
            thickness_v: config.thickness_v,
            color_hex: config.color_hex.clone(),
            border_color_hex: config.border_color_hex.clone(),
            dot_center: config.dot_center,
            opacity: config.opacity,
            border: config.border,
            border_size: config.border_size,
            space_width: config.space_width,
            rotation: config.rotation,
            dot_size: config.dot_size,
            png_crosshair: config.png_crosshair.clone(),
            mirror_crosshair: config.mirror_crosshair,
            set_monitor: config.set_monitor,
            adjust_x: config.adjust_x,
            adjust_y: config.adjust_y,
        }
    }
}

impl Profiles {
    pub fn load() -> Self {
        let path = exe_dir().join("presets.json");
        let list = match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => vec![],
        };
        Self { list, current: None }
    }

    pub fn save_to_disk(&self) {
        let path = exe_dir().join("presets.json");
        if let Ok(content) = serde_json::to_string_pretty(&self.list) {
            let _ = std::fs::write(&path, &content);
        }
    }

    pub fn apply_to_config(&self, config: &mut crate::config::Config, idx: usize) {
        if let Some(p) = self.list.get(idx) {
            p.apply_to_config(config);
        }
    }

    pub fn copy_config_to_current(&mut self, config: &crate::config::Config) {
        if let Some(i) = self.current {
            if let Some(p) = self.list.get_mut(i) {
                p.crosshair_type = config.crosshair_type.as_str().to_string();
                p.size = config.size;
                p.thickness = config.thickness;
                p.thickness_h = config.thickness_h;
                p.thickness_v = config.thickness_v;
                p.color_hex = config.color_hex.clone();
                p.border_color_hex = config.border_color_hex.clone();
                p.dot_center = config.dot_center;
                p.opacity = config.opacity;
                p.border = config.border;
                p.border_size = config.border_size;
                p.space_width = config.space_width;
                p.rotation = config.rotation;
                p.dot_size = config.dot_size;
                p.png_crosshair = config.png_crosshair.clone();
                p.mirror_crosshair = config.mirror_crosshair;
                p.set_monitor = config.set_monitor;
                p.adjust_x = config.adjust_x;
                p.adjust_y = config.adjust_y;
            }
        }
    }

    pub fn current_index_by_name(&self, name: &str) -> Option<usize> {
        self.list.iter().position(|p| p.name == name)
    }
}

pub fn load_config_with_active_profile(config: &mut crate::config::Config, profiles: &Profiles) {
    *config = crate::config::Config::load();
    if let Some(idx) = profiles.current {
        if idx < profiles.list.len() {
            profiles.apply_to_config(config, idx);
        }
    }
}
