use gpui::*;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct AppearanceSettings {
    pub image_radius: Pixels,
    pub element_radius: Pixels,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            image_radius: px(22.0),
            element_radius: px(12.0),
        }
    }
}

impl Global for AppearanceSettings {}

#[derive(serde::Serialize, serde::Deserialize)]
struct PersistedAppearanceSettings {
    image_radius: f32,
    element_radius: f32,
}

fn settings_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config/rivet/appearance.conf"))
}

fn load_saved_settings() -> Option<AppearanceSettings> {
    let path = settings_path()?;
    let raw = fs::read_to_string(path).ok()?;

    if let Ok(saved) = serde_json::from_str::<PersistedAppearanceSettings>(&raw) {
        return Some(AppearanceSettings {
            image_radius: px(saved.image_radius.clamp(0.0, 22.0)),
            element_radius: px(saved.element_radius.clamp(0.0, 24.0)),
        });
    }

    // Legacy format: a single float storing the old avatar/image radius.
    let value = raw.trim().parse::<f32>().ok()?;
    Some(AppearanceSettings {
        image_radius: px(value.clamp(0.0, 22.0)),
        ..AppearanceSettings::default()
    })
}

fn persist_settings(settings: &AppearanceSettings) {
    let Some(path) = settings_path() else {
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let saved = PersistedAppearanceSettings {
        image_radius: f32::from(settings.image_radius),
        element_radius: f32::from(settings.element_radius),
    };

    if let Ok(json) = serde_json::to_string(&saved) {
        let _ = fs::write(path, json);
    }
}

pub fn init(cx: &mut App) {
    let mut settings = AppearanceSettings::default();
    if let Some(saved_settings) = load_saved_settings() {
        settings = saved_settings;
    }
    cx.set_global(settings);
}

pub fn get_image_radius(cx: &App) -> Pixels {
    cx.global::<AppearanceSettings>().image_radius
}

pub fn get_element_radius(cx: &App) -> Pixels {
    cx.global::<AppearanceSettings>().element_radius
}

pub fn avatar_radius_for(size: Pixels, cx: &App) -> Pixels {
    let configured = f32::from(get_image_radius(cx));
    let half = f32::from(size) / 2.0;
    px(configured.min(half).max(0.0))
}

pub fn element_radius_small(cx: &App) -> Pixels {
    px((f32::from(get_element_radius(cx)) * 0.75).clamp(0.0, 18.0))
}

pub fn element_radius_large(cx: &App) -> Pixels {
    px((f32::from(get_element_radius(cx)) * 1.2).clamp(0.0, 24.0))
}

pub fn set_image_radius(radius: Pixels, cx: &mut App) {
    let radius = px(f32::from(radius).clamp(0.0, 22.0));
    let settings = cx.global_mut::<AppearanceSettings>();
    settings.image_radius = radius;
    persist_settings(settings);
    cx.refresh_windows();
}

pub fn set_element_radius(radius: Pixels, cx: &mut App) {
    let radius = px(f32::from(radius).clamp(0.0, 24.0));
    let settings = cx.global_mut::<AppearanceSettings>();
    settings.element_radius = radius;
    persist_settings(settings);
    cx.refresh_windows();
}
