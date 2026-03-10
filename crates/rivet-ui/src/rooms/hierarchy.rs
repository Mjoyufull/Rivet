use super::{RailSelection, RoomInfo};
use matrix_sdk_ui::eyeball_im::Vector;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedRoomLists {
    pub rooms: Vec<RoomInfo>,
    pub people: Vec<RoomInfo>,
    pub spaces: Vec<RoomInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomSection {
    pub heading: Option<String>,
    pub rooms: Vec<RoomInfo>,
}

pub fn derive_room_lists(
    all_rooms: &Vector<RoomInfo>,
    rail_selection: &RailSelection,
    show_rooms_in_home: bool,
) -> DerivedRoomLists {
    let mut rooms = Vec::new();
    let mut people = Vec::new();
    let mut spaces = Vec::new();

    let mut space_children_by_parent: HashMap<String, Vec<String>> = HashMap::new();
    let mut space_ids = HashSet::new();
    let mut joined_spaces = Vec::new();

    for info in all_rooms.iter() {
        if !info.is_space || !info.is_joined {
            continue;
        }

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

    for space in joined_spaces {
        if !child_space_ids.contains(&space.id) {
            spaces.push(space);
        }
    }

    let selected_space_id = match rail_selection {
        RailSelection::Space(space_id) => Some(space_id.as_str()),
        RailSelection::Home | RailSelection::Rooms => None,
    };

    let (visible_spaces, visible_room_ids) = if let Some(selected_space_id) = selected_space_id {
        let mut visible_spaces = HashSet::new();
        let mut visible_room_ids = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(selected_space_id.to_string());

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

    for info in all_rooms.iter() {
        if info.is_space {
            continue;
        }

        if info.is_direct {
            if matches!(rail_selection, RailSelection::Home) {
                people.push(info.clone());
            }
            continue;
        }

        if let (Some(visible_spaces), Some(visible_room_ids)) = (&visible_spaces, &visible_room_ids)
        {
            if visible_room_ids.contains(&info.id)
                || info
                    .parent_spaces
                    .iter()
                    .any(|parent| visible_spaces.contains(parent))
            {
                rooms.push(info.clone());
            }
        } else {
            match rail_selection {
                RailSelection::Home => {
                    if show_rooms_in_home {
                        rooms.push(info.clone());
                    }
                }
                RailSelection::Rooms => {
                    if !belongs_to_joined_space(info, &space_ids) {
                        rooms.push(info.clone());
                    }
                }
                RailSelection::Space(_) => {}
            }
        }
    }

    DerivedRoomLists {
        rooms,
        people,
        spaces,
    }
}

pub fn build_room_sections(
    rail_selection: &RailSelection,
    rooms: &[RoomInfo],
    all_rooms: &[RoomInfo],
) -> Vec<RoomSection> {
    let RailSelection::Space(selected_space_id) = rail_selection else {
        let heading = match rail_selection {
            RailSelection::Rooms => "OTHER ROOMS",
            RailSelection::Home | RailSelection::Space(_) => "ROOMS",
        };
        return vec![RoomSection {
            heading: Some(heading.to_string()),
            rooms: rooms.to_vec(),
        }];
    };

    let joined_spaces_by_id: HashMap<String, RoomInfo> = all_rooms
        .iter()
        .filter(|room| room.is_space && room.is_joined)
        .cloned()
        .map(|room| (room.id.clone(), room))
        .collect();

    let mut direct_child_spaces: Vec<RoomInfo> = Vec::new();
    let mut seen_direct_child_spaces = HashSet::new();

    if let Some(selected_space) = joined_spaces_by_id.get(selected_space_id) {
        for child_id in &selected_space.space_children {
            if let Some(space) = joined_spaces_by_id.get(child_id)
                && seen_direct_child_spaces.insert(space.id.clone())
            {
                direct_child_spaces.push(space.clone());
            }
        }
    }

    for space in joined_spaces_by_id.values() {
        if space
            .parent_spaces
            .iter()
            .any(|parent| parent == selected_space_id)
            && seen_direct_child_spaces.insert(space.id.clone())
        {
            direct_child_spaces.push(space.clone());
        }
    }

    let direct_child_space_ids: HashSet<String> = direct_child_spaces
        .iter()
        .map(|space| space.id.clone())
        .collect();

    let mut root_rooms = Vec::new();
    let mut grouped_rooms: HashMap<String, Vec<RoomInfo>> = HashMap::new();

    for room in rooms.iter().cloned() {
        if let Some(child_space_id) = resolve_direct_child_space(
            &room,
            selected_space_id,
            &direct_child_space_ids,
            &joined_spaces_by_id,
        ) {
            grouped_rooms.entry(child_space_id).or_default().push(room);
        } else {
            root_rooms.push(room);
        }
    }

    let mut sections = vec![RoomSection {
        heading: Some("ROOMS".to_string()),
        rooms: root_rooms,
    }];

    for child_space in direct_child_spaces {
        if let Some(space_rooms) = grouped_rooms.remove(&child_space.id) {
            if !space_rooms.is_empty() {
                sections.push(RoomSection {
                    heading: Some(child_space.name.to_uppercase()),
                    rooms: space_rooms,
                });
            }
        }
    }

    sections
}

fn resolve_direct_child_space(
    room: &RoomInfo,
    selected_space_id: &str,
    direct_child_space_ids: &HashSet<String>,
    joined_spaces_by_id: &HashMap<String, RoomInfo>,
) -> Option<String> {
    let mut queue: VecDeque<String> = room.parent_spaces.iter().cloned().collect();
    let mut visited = HashSet::new();

    while let Some(space_id) = queue.pop_front() {
        if !visited.insert(space_id.clone()) {
            continue;
        }

        if space_id == selected_space_id {
            continue;
        }

        if direct_child_space_ids.contains(&space_id) {
            return Some(space_id);
        }

        if let Some(space) = joined_spaces_by_id.get(&space_id) {
            for parent in &space.parent_spaces {
                queue.push_back(parent.clone());
            }
        }
    }

    None
}

fn belongs_to_joined_space(room: &RoomInfo, joined_space_ids: &HashSet<String>) -> bool {
    room.parent_spaces
        .iter()
        .any(|parent_id| joined_space_ids.contains(parent_id))
}
