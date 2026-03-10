use gpui::*;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct UiPreferences {
    pub show_rooms_in_home: bool,
    pub show_sidecart: bool,
    pub show_other_rooms: bool,
    pub remember_last_room: bool,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            show_rooms_in_home: false,
            show_sidecart: true,
            show_other_rooms: true,
            remember_last_room: true,
        }
    }
}

impl Global for UiPreferences {}

#[derive(serde::Serialize, serde::Deserialize)]
struct PersistedUiPreferences {
    show_rooms_in_home: bool,
    show_sidecart: bool,
    show_other_rooms: bool,
    remember_last_room: bool,
}

fn settings_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config/rivet/ui.conf"))
}

fn load_saved_preferences() -> Option<UiPreferences> {
    let path = settings_path()?;
    let raw = fs::read_to_string(path).ok()?;
    let saved = serde_json::from_str::<PersistedUiPreferences>(&raw).ok()?;

    Some(UiPreferences {
        show_rooms_in_home: saved.show_rooms_in_home,
        show_sidecart: saved.show_sidecart,
        show_other_rooms: saved.show_other_rooms,
        remember_last_room: saved.remember_last_room,
    })
}

fn persist_preferences(preferences: &UiPreferences) {
    let Some(path) = settings_path() else {
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let saved = PersistedUiPreferences {
        show_rooms_in_home: preferences.show_rooms_in_home,
        show_sidecart: preferences.show_sidecart,
        show_other_rooms: preferences.show_other_rooms,
        remember_last_room: preferences.remember_last_room,
    };

    if let Ok(json) = serde_json::to_string(&saved) {
        let _ = fs::write(path, json);
    }
}

pub fn init(cx: &mut App) {
    let preferences = load_saved_preferences().unwrap_or_default();
    cx.set_global(preferences);
}

pub fn ui_preferences(cx: &App) -> UiPreferences {
    cx.global::<UiPreferences>().clone()
}

pub fn set_show_rooms_in_home(show_rooms_in_home: bool, cx: &mut App) {
    let preferences = cx.global_mut::<UiPreferences>();
    preferences.show_rooms_in_home = show_rooms_in_home;
    persist_preferences(preferences);
}

pub fn set_show_sidecart(show_sidecart: bool, cx: &mut App) {
    let preferences = cx.global_mut::<UiPreferences>();
    preferences.show_sidecart = show_sidecart;
    persist_preferences(preferences);
    cx.refresh_windows();
}

pub fn set_show_other_rooms(show_other_rooms: bool, cx: &mut App) {
    let preferences = cx.global_mut::<UiPreferences>();
    preferences.show_other_rooms = show_other_rooms;
    persist_preferences(preferences);
    cx.refresh_windows();
}

pub fn set_remember_last_room(remember_last_room: bool, cx: &mut App) {
    let preferences = cx.global_mut::<UiPreferences>();
    preferences.remember_last_room = remember_last_room;
    persist_preferences(preferences);
}
