mod render;
mod security;
mod session;
mod settings;

use crate::auth::LoginView;
use crate::components::chat::{ChatView, DetailsPanelPreferences};
use crate::rooms::RoomListModel;
use crate::security::verification::VerificationModel;
use crate::settings::SettingsView;
use crate::sidebar::Sidebar;
use crate::timeline::TimelineModel;
use gpui::*;
use rivet_core::client::RivetClient;

pub(crate) struct AppView {
    sidebar: Entity<Sidebar>,
    login_view: Entity<LoginView>,
    settings_view: Entity<SettingsView>,
    is_logged_in: bool,
    is_settings_open: bool,
    client: Option<RivetClient>,
    pub(crate) room_list_model: Option<Entity<RoomListModel>>,
    pub(crate) active_timeline_model: Option<Entity<TimelineModel>>,
    active_chat_view: Option<Entity<ChatView>>,
    pub(crate) details_panel_preferences: DetailsPanelPreferences,
    verification_model: Option<Entity<VerificationModel>>,
    verification_gate_active: bool,
    active_room_id: Option<String>,
    sync_status: String,
    session_verified: bool,
    pub(crate) sidebar_width: Pixels,
    is_recovering: bool,
    recovery_status: Option<Result<(), String>>,
}

impl AppView {
    pub(crate) fn new(
        sidebar: Entity<Sidebar>,
        login_view: Entity<LoginView>,
        settings_view: Entity<SettingsView>,
    ) -> Self {
        Self {
            sidebar,
            login_view,
            settings_view,
            is_logged_in: false,
            is_settings_open: false,
            client: None,
            room_list_model: None,
            active_timeline_model: None,
            active_chat_view: None,
            details_panel_preferences: DetailsPanelPreferences::default(),
            verification_model: None,
            verification_gate_active: false,
            active_room_id: None,
            sync_status: "Idle".to_string(),
            session_verified: false,
            sidebar_width: px(268.0),
            is_recovering: false,
            recovery_status: None,
        }
    }

    fn is_initial_sync_complete(&self, cx: &App) -> bool {
        self.room_list_model
            .as_ref()
            .map(|model| !model.read(cx).is_loading)
            .unwrap_or(false)
    }

    fn set_session_verified(&mut self, verified: bool, cx: &mut Context<Self>) {
        if self.session_verified == verified {
            return;
        }

        self.session_verified = verified;
        self.settings_view.update(cx, |settings, cx| {
            settings.set_session_verified(verified, cx);
        });
        cx.notify();
    }
}
