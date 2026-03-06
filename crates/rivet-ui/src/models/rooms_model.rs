use futures::StreamExt;
use gpui::*;
use gpui::{App, AsyncApp, Entity};
use matrix_sdk::RoomDisplayName;
use matrix_sdk::RoomMemberships;
use matrix_sdk::RoomState;
use matrix_sdk::deserialized_responses::SyncOrStrippedState;
use matrix_sdk::room::ParentSpace;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::SyncStateEvent;
use matrix_sdk::ruma::events::space::child::SpaceChildEventContent;
use matrix_sdk_ui::eyeball_im::{Vector, VectorDiff};
use matrix_sdk_ui::room_list_service::RoomListItem;
use rivet_core::client::RivetClient;
use std::collections::{HashMap, HashSet, VecDeque};

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
        cx.spawn(|cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                tracing::info!("Initializing Reactive RoomListModel");

                let room_list_service = client.room_list_service().await;
                if let Some(service) = room_list_service {
                    tracing::info!("Got RoomListService, getting all_rooms...");

                    // Get all rooms list
                    let room_list = match service.all_rooms().await {
                        Ok(list) => {
                            tracing::info!("Got room list successfully");
                            list
                        }
                        Err(e) => {
                            tracing::error!("Failed to get all_rooms: {:?}", e);
                            return;
                        }
                    };

                    let (stream, controller) = room_list.entries_with_dynamic_adapters(100);

                    // INITIALIZE FILTER TO START THE STREAM
                    // In matrix-sdk-ui 0.16.x, the stream won't yield anything until a filter is set.
                    controller.set_filter(Box::new(|_| true));

                    model.update(&mut cx, |this, _| {
                        this._room_list_controller = Some(controller);
                    });

                    let mut stream = std::pin::pin!(stream);

                    tracing::info!("Starting room list stream listener...");

                    // Listen for real-time updates
                    while let Some(diffs) = stream.next().await {
                        tracing::info!("Received {} diffs from room list stream", diffs.len());

                        let mut items_to_process = Vec::new();
                        for diff in diffs {
                            match diff {
                                VectorDiff::Reset { values } => {
                                    tracing::info!("VectorDiff::Reset with {} rooms", values.len());
                                    let processed =
                                        Self::process_room_items(values.into_iter().collect())
                                            .await;
                                    cx.update(|cx| {
                                        let _ = model.update(cx, |this, cx| {
                                            this.apply_diff(VectorDiff::Reset {
                                                values: processed.into(),
                                            });
                                            cx.notify();
                                        });
                                    });
                                }
                                VectorDiff::Append { values } => {
                                    let processed =
                                        Self::process_room_items(values.into_iter().collect())
                                            .await;
                                    cx.update(|cx| {
                                        let _ = model.update(cx, |this, cx| {
                                            this.apply_diff(VectorDiff::Append {
                                                values: processed.into(),
                                            });
                                            cx.notify();
                                        });
                                    });
                                }
                                // For other variants, we might want to batch them too, but they are usually single items.
                                // Let's keep the existing logic but wrap in model.update
                                _ => {
                                    items_to_process.push(diff);
                                }
                            }
                        }

                        if !items_to_process.is_empty() {
                            let mut processed_diffs = Vec::new();

                            for diff in items_to_process {
                                match diff {
                                    VectorDiff::PushBack { value } => {
                                        let processed = Self::process_room_items(vec![value]).await;
                                        if let Some(item) = processed.into_iter().next() {
                                            processed_diffs
                                                .push(VectorDiff::PushBack { value: item });
                                        }
                                    }
                                    VectorDiff::PushFront { value } => {
                                        let processed = Self::process_room_items(vec![value]).await;
                                        if let Some(item) = processed.into_iter().next() {
                                            processed_diffs
                                                .push(VectorDiff::PushFront { value: item });
                                        }
                                    }
                                    VectorDiff::Insert { index, value } => {
                                        let processed = Self::process_room_items(vec![value]).await;
                                        if let Some(item) = processed.into_iter().next() {
                                            processed_diffs
                                                .push(VectorDiff::Insert { index, value: item });
                                        }
                                    }
                                    VectorDiff::Set { index, value } => {
                                        let processed = Self::process_room_items(vec![value]).await;
                                        if let Some(item) = processed.into_iter().next() {
                                            processed_diffs
                                                .push(VectorDiff::Set { index, value: item });
                                        }
                                    }
                                    VectorDiff::Remove { index } => {
                                        processed_diffs.push(VectorDiff::Remove { index });
                                    }
                                    VectorDiff::PopFront => {
                                        processed_diffs.push(VectorDiff::PopFront);
                                    }
                                    VectorDiff::PopBack => {
                                        processed_diffs.push(VectorDiff::PopBack);
                                    }
                                    VectorDiff::Clear => {
                                        processed_diffs.push(VectorDiff::Clear);
                                    }
                                    VectorDiff::Truncate { length } => {
                                        processed_diffs.push(VectorDiff::Truncate { length });
                                    }
                                    _ => {}
                                }
                            }

                            if !processed_diffs.is_empty() {
                                let _ = cx.update(|cx| {
                                    let _ = model.update(cx, |this, cx| {
                                        for diff in processed_diffs {
                                            this.apply_diff(diff);
                                        }
                                        cx.notify();
                                    });
                                });
                            }
                        }
                    }
                    tracing::warn!("Room list stream ended unexpectedly");
                } else {
                    tracing::error!("No RoomListService available!");
                }
            }
        })
        .detach();
    }

    async fn process_room_items(items: Vec<RoomListItem>) -> Vec<RoomInfo> {
        let mut room_infos = Vec::new();
        for item in items {
            let room = item.into_inner().clone();
            let room_id = room.room_id();
            let room_id_str = room_id.to_string();

            let mut name = match room.display_name().await {
                Ok(dn) => match dn {
                    RoomDisplayName::Named(s)
                    | RoomDisplayName::Calculated(s)
                    | RoomDisplayName::Aliased(s) => s,
                    RoomDisplayName::EmptyWas(s) => format!("Empty Room (was {})", s),
                    RoomDisplayName::Empty => "Empty Room".to_string(),
                },
                Err(_) => room_id_str.clone(),
            };

            let is_direct = room.is_direct().await.unwrap_or(false);
            let unread_counts = room.unread_notification_counts();
            let is_space = room.is_space();
            let is_joined = room.state() == RoomState::Joined;
            let mut avatar_url = room.avatar_url().map(|u| u.to_string());

            // For DMs, rooms often don't have a room avatar. Pull the other member's profile
            // from local state (no sync) for a better avatar/name fallback.
            if is_direct && avatar_url.is_none() {
                if let Ok(members) = room.members_no_sync(RoomMemberships::ACTIVE).await {
                    if let Some(other) = members
                        .into_iter()
                        .find(|m| m.user_id() != room.own_user_id())
                    {
                        if avatar_url.is_none() {
                            avatar_url = other.avatar_url().map(|u| u.to_string());
                        }

                        if name.starts_with('@')
                            || name == room_id_str
                            || name.starts_with("Empty Room")
                        {
                            if let Some(display_name) = other.display_name() {
                                if !display_name.trim().is_empty() {
                                    name = display_name.to_string();
                                }
                            } else {
                                name = other.user_id().localpart().to_string();
                            }
                        }
                    }
                }
            }

            if is_direct && name.starts_with('@') {
                name = name
                    .trim_start_matches('@')
                    .split(':')
                    .next()
                    .unwrap_or(&name)
                    .to_string();
            }

            // Keep room list processing snappy: avoid per-room member fetches here.
            // Timeline sender profiles provide avatars for message rows.

            let mut parent_spaces = Vec::new();
            if let Ok(mut spaces_stream) = room.parent_spaces().await {
                while let Some(Ok(parent)) = spaces_stream.next().await {
                    let id = match parent {
                        ParentSpace::Reciprocal(r) => r.room_id().to_owned(),
                        ParentSpace::WithPowerlevel(r) => r.room_id().to_owned(),
                        ParentSpace::Illegitimate(r) => r.room_id().to_owned(),
                        ParentSpace::Unverifiable(id) => id,
                    };
                    parent_spaces.push(id.to_string());
                }
            }

            let mut space_children = Vec::new();
            if is_space
                && let Ok(children) = room
                    .get_state_events_static::<SpaceChildEventContent>()
                    .await
            {
                for child_event in children {
                    let state_key: Option<OwnedRoomId> = match child_event.deserialize() {
                        Ok(SyncOrStrippedState::Sync(SyncStateEvent::Original(e))) => {
                            Some(e.state_key.to_owned())
                        }
                        Ok(SyncOrStrippedState::Stripped(e)) => Some(e.state_key.to_owned()),
                        _ => None,
                    };

                    if let Some(state_key) = state_key {
                        space_children.push(state_key.to_string());
                    }
                }
            }

            room_infos.push(RoomInfo {
                id: room_id.to_string(),
                name,
                avatar_url,
                unread_count: unread_counts.notification_count,
                highlight_count: unread_counts.highlight_count,
                is_space,
                is_direct,
                is_joined,
                parent_spaces,
                space_children,
            });
        }
        room_infos
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
        let mut rooms = Vec::new();
        let mut people = Vec::new();
        let mut spaces = Vec::new();

        let mut space_children_by_parent: HashMap<String, Vec<String>> = HashMap::new();
        let mut space_ids = HashSet::new();
        let mut joined_spaces = Vec::new();

        for info in self.all_rooms.iter() {
            if info.is_space {
                // Track only joined spaces for graph computation and rail rendering.
                if info.is_joined {
                    joined_spaces.push(info.clone());
                    space_ids.insert(info.id.clone());

                    for child in &info.space_children {
                        space_children_by_parent
                            .entry(info.id.clone())
                            .or_default()
                            .push(child.clone());
                    }

                    for parent in &info.parent_spaces {
                        space_children_by_parent
                            .entry(parent.clone())
                            .or_default()
                            .push(info.id.clone());
                    }
                }
            }
        }

        // Show only top-level joined spaces in the rail.
        // A child space is any joined space that appears as a child of another joined space
        // either via explicit m.space.child or via parent fallback edges.
        let mut child_space_ids = HashSet::new();
        for (parent, children) in &space_children_by_parent {
            if !space_ids.contains(parent) {
                continue;
            }
            for child in children {
                if space_ids.contains(child) {
                    child_space_ids.insert(child.clone());
                }
            }
        }

        // Child spaces are represented as sections under ROOMS when a top-level space is selected.
        for space in joined_spaces {
            if !child_space_ids.contains(&space.id) {
                spaces.push(space);
            }
        }

        let (visible_spaces, visible_room_ids) =
            if let Some(selected_space_id) = &self.selected_space_id {
                let mut visible_spaces = HashSet::new();
                let mut visible_room_ids = HashSet::new();
                let mut queue = VecDeque::new();
                queue.push_back(selected_space_id.clone());

                while let Some(space_id) = queue.pop_front() {
                    if !visible_spaces.insert(space_id.clone()) {
                        continue;
                    }

                    if let Some(children) = space_children_by_parent.get(&space_id) {
                        for child in children {
                            if space_ids.contains(child) {
                                queue.push_back(child.clone());
                            } else {
                                visible_room_ids.insert(child.clone());
                            }
                        }
                    }
                }

                (Some(visible_spaces), Some(visible_room_ids))
            } else {
                (None, None)
            };

        for info in self.all_rooms.iter() {
            if info.is_space {
                continue;
            }

            if info.is_direct {
                if self.selected_space_id.is_none() {
                    people.push(info.clone());
                }
                continue;
            }

            if let (Some(visible_spaces), Some(visible_room_ids)) =
                (&visible_spaces, &visible_room_ids)
            {
                if visible_room_ids.contains(&info.id)
                    || info
                        .parent_spaces
                        .iter()
                        .any(|parent| visible_spaces.contains(parent))
                {
                    rooms.push(info.clone());
                }
            } else if self.show_rooms_in_home {
                rooms.push(info.clone());
            }
        }

        self.rooms = rooms;
        self.people = people;
        self.spaces = spaces;
        self.is_loading = false;
    }
}
