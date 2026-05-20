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
    pub color_hex: String,
    pub dot_center: bool,
    pub opacity: f32,
    pub border: bool,
    pub space_width: f32,
    pub rotation: f32,
    pub dot_size: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub png_crosshair: Option<String>,
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
    pub fn from_config(name: String, config: &crate::config::Config) -> Self {
        Self {
            name,
            game_exe: None,
            crosshair_type: config.crosshair_type.as_str().to_string(),
            size: config.size,
            thickness: config.thickness,
            color_hex: config.color_hex.clone(),
            dot_center: config.dot_center,
            opacity: config.opacity,
            border: config.border,
            space_width: config.space_width,
            rotation: config.rotation,
            dot_size: config.dot_size,
            png_crosshair: config.png_crosshair.clone(),
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
            config.crosshair_type = crate::config::CrosshairType::from_str(&p.crosshair_type);
            config.size = p.size;
            config.thickness = p.thickness;
            config.color_hex = p.color_hex.clone();
            config.dot_center = p.dot_center;
            config.opacity = p.opacity;
            config.border = p.border;
            config.space_width = p.space_width;
            config.rotation = p.rotation;
            config.dot_size = p.dot_size;
            config.png_crosshair = p.png_crosshair.clone();
        }
    }

    pub fn copy_config_to_current(&mut self, config: &crate::config::Config) {
        if let Some(i) = self.current {
            if let Some(p) = self.list.get_mut(i) {
                p.crosshair_type = config.crosshair_type.as_str().to_string();
                p.size = config.size;
                p.thickness = config.thickness;
                p.color_hex = config.color_hex.clone();
                p.dot_center = config.dot_center;
                p.opacity = config.opacity;
                p.border = config.border;
                p.space_width = config.space_width;
                p.rotation = config.rotation;
                p.dot_size = config.dot_size;
                p.png_crosshair = config.png_crosshair.clone();
            }
        }
    }

    pub fn current_index_by_name(&self, name: &str) -> Option<usize> {
        self.list.iter().position(|p| p.name == name)
    }
}
