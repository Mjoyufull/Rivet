mod enrichment;
mod stream;

use super::hierarchy::derive_room_lists;
use gpui::*;
use matrix_sdk_ui::eyeball_im::{Vector, VectorDiff};
use rivet_core::client::RivetClient;

#[derive(Clone, Default, Debug)]
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
    pub selected_space_id: Option<String>,
    pub show_rooms_in_home: bool,
    _room_list_controller:
        Option<matrix_sdk_ui::room_list_service::RoomListDynamicEntriesController>,
}

impl EventEmitter<()> for RoomListModel {}

impl RoomListModel {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            rooms: Vec::new(),
            people: Vec::new(),
            spaces: Vec::new(),
            all_rooms: Vector::new(),
            is_loading: true,
            selected_room_id: None,
            selected_space_id: None,
            show_rooms_in_home: false,
            _room_list_controller: None,
        })
    }

    pub fn select_room(&mut self, room_id: String, cx: &mut Context<Self>) {
        if self.selected_room_id.as_ref() != Some(&room_id) {
            self.selected_room_id = Some(room_id);
            cx.notify();
        }
    }

    pub fn select_space(&mut self, space_id: Option<String>, cx: &mut Context<Self>) {
        if self.selected_space_id != space_id {
            self.selected_space_id = space_id;
            self.recalculate_derived_lists();
            cx.notify();
        }
    }

    pub fn set_show_rooms_in_home(&mut self, val: bool, cx: &mut Context<Self>) {
        if self.show_rooms_in_home != val {
            self.show_rooms_in_home = val;
            self.recalculate_derived_lists();
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
            self.selected_space_id.as_deref(),
            self.show_rooms_in_home,
        );

        self.rooms = derived.rooms;
        self.people = derived.people;
        self.spaces = derived.spaces;
        self.is_loading = false;
    }
}
