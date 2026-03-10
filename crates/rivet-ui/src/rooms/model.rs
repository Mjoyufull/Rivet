mod enrichment;
mod stream;

use super::hierarchy::derive_room_lists;
use crate::models::ui_preferences;
use gpui::*;
use matrix_sdk_ui::eyeball_im::{Vector, VectorDiff};
use rivet_core::client::RivetClient;
use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum RailSelection {
    #[default]
    Home,
    Rooms,
    Space(String),
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct RoomInfo {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub unread_count: u64,
    pub highlight_count: u64,
    pub is_space: bool,
    pub is_direct: bool,
    pub is_joined: bool,
    pub parent_spaces: Vec<String>,
    pub space_children: Vec<String>,
}

pub struct RoomListModel {
    pub rooms: Vec<RoomInfo>,
    pub people: Vec<RoomInfo>,
    pub spaces: Vec<RoomInfo>,
    pub all_rooms: Vector<RoomInfo>,
    pub is_loading: bool,
    pub selected_room_id: Option<String>,
    pub rail_selection: RailSelection,
    pub show_rooms_in_home: bool,
    pub show_other_rooms: bool,
    pub remember_last_room: bool,
    last_rooms_room_id: Option<String>,
    last_space_room_ids: HashMap<String, String>,
    _room_list_controller:
        Option<matrix_sdk_ui::room_list_service::RoomListDynamicEntriesController>,
}

impl EventEmitter<()> for RoomListModel {}

impl RoomListModel {
    pub fn new(cx: &mut App) -> Entity<Self> {
        let preferences = ui_preferences::ui_preferences(cx);
        cx.new(|_| Self {
            rooms: Vec::new(),
            people: Vec::new(),
            spaces: Vec::new(),
            all_rooms: Vector::new(),
            is_loading: true,
            selected_room_id: None,
            rail_selection: RailSelection::Home,
            show_rooms_in_home: preferences.show_rooms_in_home,
            show_other_rooms: preferences.show_other_rooms,
            remember_last_room: preferences.remember_last_room,
            last_rooms_room_id: None,
            last_space_room_ids: HashMap::new(),
            _room_list_controller: None,
        })
    }

    pub fn select_room(&mut self, room_id: String, cx: &mut Context<Self>) {
        if self.selected_room_id.as_ref() != Some(&room_id) {
            match &self.rail_selection {
                RailSelection::Rooms => self.last_rooms_room_id = Some(room_id.clone()),
                RailSelection::Space(space_id) => {
                    self.last_space_room_ids
                        .insert(space_id.clone(), room_id.clone());
                }
                RailSelection::Home => {}
            }
            self.selected_room_id = Some(room_id);
            cx.notify();
        }
    }

    pub fn clear_selected_room(&mut self, cx: &mut Context<Self>) {
        if self.selected_room_id.take().is_some() {
            cx.notify();
        }
    }

    pub fn select_home(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        if self.rail_selection != RailSelection::Home {
            self.rail_selection = RailSelection::Home;
            self.recalculate_derived_lists();
            changed = true;
        }
        if self.selected_room_id.take().is_some() {
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }

    pub fn select_rooms(&mut self, cx: &mut Context<Self>) {
        if !self.show_other_rooms {
            return;
        }

        if self.rail_selection != RailSelection::Rooms {
            self.rail_selection = RailSelection::Rooms;
            self.recalculate_derived_lists();
            self.restore_last_room_for_current_selection(cx);
        } else if self.remember_last_room {
            self.restore_last_room_for_current_selection(cx);
        } else if self.selected_room_id.take().is_some() {
            cx.notify();
        }
    }

    pub fn select_space(&mut self, space_id: String, cx: &mut Context<Self>) {
        let target = RailSelection::Space(space_id);
        if self.rail_selection != target {
            self.rail_selection = target;
            self.recalculate_derived_lists();
            self.restore_last_room_for_current_selection(cx);
        } else if self.remember_last_room {
            self.restore_last_room_for_current_selection(cx);
        } else if self.selected_room_id.take().is_some() {
            cx.notify();
        }
    }

    pub fn set_show_rooms_in_home(&mut self, val: bool, cx: &mut Context<Self>) {
        if self.show_rooms_in_home != val {
            self.show_rooms_in_home = val;
            ui_preferences::set_show_rooms_in_home(val, cx);
            self.recalculate_derived_lists();
            cx.notify();
        }
    }

    pub fn set_show_other_rooms(&mut self, val: bool, cx: &mut Context<Self>) {
        if self.show_other_rooms != val {
            self.show_other_rooms = val;
            ui_preferences::set_show_other_rooms(val, cx);
            if !val && matches!(self.rail_selection, RailSelection::Rooms) {
                self.rail_selection = RailSelection::Home;
            }
            self.recalculate_derived_lists();
            cx.notify();
        }
    }

    pub fn set_remember_last_room(&mut self, val: bool, cx: &mut Context<Self>) {
        if self.remember_last_room != val {
            self.remember_last_room = val;
            ui_preferences::set_remember_last_room(val, cx);
            if !val {
                self.last_rooms_room_id = None;
                self.last_space_room_ids.clear();
            }
            cx.notify();
        }
    }

    fn restore_last_room_for_current_selection(&mut self, cx: &mut Context<Self>) {
        if !self.remember_last_room {
            self.selected_room_id = None;
            cx.notify();
            return;
        }

        let candidate = match &self.rail_selection {
            RailSelection::Rooms => self.last_rooms_room_id.clone(),
            RailSelection::Space(space_id) => self.last_space_room_ids.get(space_id).cloned(),
            RailSelection::Home => None,
        };

        if let Some(room_id) = candidate {
            if self.rooms.iter().any(|room| room.id == room_id) {
                self.selected_room_id = Some(room_id);
                cx.notify();
                return;
            }
        }

        if self.selected_room_id.take().is_some()
            || !matches!(self.rail_selection, RailSelection::Home)
        {
            cx.notify();
        }
    }

    pub fn init(model: Entity<Self>, client: RivetClient, cx: &mut App) {
        stream::init(model, client, cx);
    }

    pub fn apply_diff(&mut self, diff: VectorDiff<RoomInfo>) {
        match diff {
            VectorDiff::Append { values } => {
                self.all_rooms.append(values);
            }
            VectorDiff::Clear => {
                self.all_rooms.clear();
            }
            VectorDiff::PushFront { value } => {
                self.all_rooms.push_front(value);
            }
            VectorDiff::PushBack { value } => {
                self.all_rooms.push_back(value);
            }
            VectorDiff::PopFront => {
                self.all_rooms.pop_front();
            }
            VectorDiff::PopBack => {
                self.all_rooms.pop_back();
            }
            VectorDiff::Insert { index, value } => {
                if index <= self.all_rooms.len() {
                    self.all_rooms.insert(index, value);
                }
            }
            VectorDiff::Set { index, value } => {
                if index < self.all_rooms.len() {
                    self.all_rooms.set(index, value);
                }
            }
            VectorDiff::Remove { index } => {
                if index < self.all_rooms.len() {
                    self.all_rooms.remove(index);
                }
            }
            VectorDiff::Truncate { length } => {
                self.all_rooms.truncate(length);
            }
            VectorDiff::Reset { values } => {
                self.all_rooms = values;
            }
        }
        self.recalculate_derived_lists();
    }

    fn recalculate_derived_lists(&mut self) {
        let derived = derive_room_lists(
            &self.all_rooms,
            &self.rail_selection,
            self.show_rooms_in_home,
        );

        self.rooms = derived.rooms;
        self.people = derived.people;
        self.spaces = derived.spaces;
        self.is_loading = false;
    }
}
