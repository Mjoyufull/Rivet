use matrix_sdk_ui::eyeball_im::Vector;
use rivet_ui::rooms::{RailSelection, RoomInfo, build_room_sections, derive_room_lists};

fn room(id: &str, name: &str) -> RoomInfo {
    RoomInfo {
        id: id.to_string(),
        name: name.to_string(),
        is_joined: true,
        ..RoomInfo::default()
    }
}

fn direct(id: &str, name: &str) -> RoomInfo {
    RoomInfo {
        is_direct: true,
        ..room(id, name)
    }
}

fn space(id: &str, name: &str) -> RoomInfo {
    RoomInfo {
        is_space: true,
        ..room(id, name)
    }
}

fn vector(items: Vec<RoomInfo>) -> Vector<RoomInfo> {
    items.into_iter().collect()
}

#[test]
fn derive_room_lists_home_separates_people_rooms_and_top_level_spaces() {
    let mut top_space = space("!space:example.org", "Matrix Community");
    top_space.space_children = vec![
        "!room-a:example.org".into(),
        "!child-space:example.org".into(),
    ];

    let mut child_space = space("!child-space:example.org", "Chat Clients");
    child_space.parent_spaces = vec!["!space:example.org".into()];

    let mut room_in_space = room("!room-a:example.org", "Element Web Development");
    room_in_space.parent_spaces = vec!["!space:example.org".into()];

    let dm = direct("!dm:example.org", "Wired");
    let loose_room = room("!loose:example.org", "Random");

    let derived = derive_room_lists(
        &vector(vec![
            top_space.clone(),
            child_space,
            room_in_space,
            dm.clone(),
            loose_room,
        ]),
        &RailSelection::Home,
        false,
    );

    assert_eq!(derived.people, vec![dm]);
    assert!(derived.rooms.is_empty());
    assert_eq!(derived.spaces, vec![top_space]);
}

#[test]
fn derive_room_lists_rooms_only_shows_rooms_outside_spaces() {
    let top_space = space("!space:example.org", "Matrix Community");
    let mut room_in_space = room("!room-a:example.org", "Element Web Development");
    room_in_space.parent_spaces = vec!["!space:example.org".into()];
    let loose_room = room("!loose:example.org", "Random");

    let derived = derive_room_lists(
        &vector(vec![top_space, room_in_space, loose_room.clone()]),
        &RailSelection::Rooms,
        false,
    );

    assert_eq!(derived.rooms, vec![loose_room]);
    assert!(derived.people.is_empty());
}

#[test]
fn build_room_sections_groups_rooms_under_direct_child_space_headers() {
    let mut top_space = space("!space:example.org", "Matrix Community");
    top_space.space_children = vec![
        "!root-room:example.org".into(),
        "!child-space:example.org".into(),
    ];

    let mut child_space = space("!child-space:example.org", "Chat Clients");
    child_space.parent_spaces = vec!["!space:example.org".into()];
    child_space.space_children = vec!["!child-room:example.org".into()];

    let mut root_room = room("!root-room:example.org", "Element Web Development");
    root_room.parent_spaces = vec!["!space:example.org".into()];

    let mut child_room = room("!child-room:example.org", "Element Themes");
    child_room.parent_spaces = vec!["!child-space:example.org".into()];

    let sections = build_room_sections(
        &RailSelection::Space("!space:example.org".into()),
        &[root_room.clone(), child_room.clone()],
        &[top_space, child_space],
    );

    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].heading.as_deref(), Some("ROOMS"));
    assert_eq!(sections[0].rooms, vec![root_room]);
    assert_eq!(sections[1].heading.as_deref(), Some("CHAT CLIENTS"));
    assert_eq!(sections[1].rooms, vec![child_room]);
}
