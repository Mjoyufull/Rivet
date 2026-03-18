mod footer;
mod rail;
mod view;

use crate::rooms::RoomListModel;
use gpui::*;
use rivet_core::client::RivetClient;

pub(crate) use rail::SpacesRail;

pub enum SidebarEvent {
    OpenSettings,
}

#[derive(Clone)]
pub struct Sidebar {
    pub room_list_model: Option<Entity<RoomListModel>>,
    pub user_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub sync_status: String,
    pub scroll_handle: UniformListScrollHandle,
}

impl Sidebar {
    pub fn new(
        client: Option<RivetClient>,
        room_list_model: Option<Entity<RoomListModel>>,
    ) -> Self {
        let user_id = client
            .as_ref()
            .and_then(|c| c.user_id())
            .unwrap_or_else(|| "@user:matrix.org".to_string());

        let display_name = user_id
            .strip_prefix('@')
            .and_then(|s| s.split(':').next())
            .unwrap_or("User")
            .to_string();

        Self {
            room_list_model,
            user_id,
            display_name,
            avatar_url: None,
            sync_status: "Idle".to_string(),
            scroll_handle: UniformListScrollHandle::new(),
        }
    }
}

impl EventEmitter<SidebarEvent> for Sidebar {}
