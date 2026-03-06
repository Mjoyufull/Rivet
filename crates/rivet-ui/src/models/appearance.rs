use gpui::*;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct AppearanceSettings {
    pub avatar_radius: Pixels,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            avatar_radius: px(22.0), // Full circle for 44px avatars
        }
    }
}

impl Global for AppearanceSettings {}

fn settings_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config/rivet/appearance.conf"))
}

fn load_saved_radius() -> Option<Pixels> {
    let path = settings_path()?;
    let raw = fs::read_to_string(path).ok()?;
    let value = raw.trim().parse::<f32>().ok()?;
    Some(px(value.clamp(0.0, 22.0)))
}

fn persist_radius(radius: Pixels) {
    let Some(path) = settings_path() else {
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let _ = fs::write(path, format!("{:.2}", f32::from(radius)));
}

pub fn init(cx: &mut App) {
    let mut settings = AppearanceSettings::default();
    if let Some(saved_radius) = load_saved_radius() {
        settings.avatar_radius = saved_radius;
    }
    cx.set_global(settings);
}

pub fn get_radius(cx: &App) -> Pixels {
    cx.global::<AppearanceSettings>().avatar_radius
}

pub fn avatar_radius_for(size: Pixels, cx: &App) -> Pixels {
    let configured = f32::from(get_radius(cx));
    let half = f32::from(size) / 2.0;
    px(configured.min(half).max(0.0))
}

pub fn set_radius(radius: Pixels, cx: &mut App) {
    let radius = px(f32::from(radius).clamp(0.0, 22.0));
    cx.global_mut::<AppearanceSettings>().avatar_radius = radius;
    persist_radius(radius);
    cx.refresh_windows();
}

pub fn update_radius(f: impl FnOnce(Pixels) -> Pixels, cx: &mut App) {
    let settings = cx.global_mut::<AppearanceSettings>();
    let new_radius = px(f32::from(f(settings.avatar_radius)).clamp(0.0, 22.0));
    settings.avatar_radius = new_radius;
    persist_radius(new_radius);
    cx.refresh_windows();
}
