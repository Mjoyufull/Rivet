use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use matrix_sdk_ui::eyeball_im::Vector;
use rivet_ui::rooms::{RailSelection, RoomInfo, build_room_sections, derive_room_lists};

fn room(id: usize, parent_spaces: Vec<String>) -> RoomInfo {
    RoomInfo {
        id: format!("!room-{id}:example.org"),
        name: format!("Room {id}"),
        avatar_url: None,
        unread_count: 0,
        highlight_count: 0,
        is_space: false,
        is_direct: false,
        is_joined: true,
        parent_spaces,
        space_children: Vec::new(),
    }
}

fn dm(id: usize) -> RoomInfo {
    RoomInfo {
        id: format!("!dm-{id}:example.org"),
        name: format!("DM {id}"),
        avatar_url: None,
        unread_count: 0,
        highlight_count: 0,
        is_space: false,
        is_direct: true,
        is_joined: true,
        parent_spaces: Vec::new(),
        space_children: Vec::new(),
    }
}

fn space(id: usize, parents: Vec<String>, children: Vec<String>) -> RoomInfo {
    RoomInfo {
        id: format!("!space-{id}:example.org"),
        name: format!("Space {id}"),
        avatar_url: None,
        unread_count: 0,
        highlight_count: 0,
        is_space: true,
        is_direct: false,
        is_joined: true,
        parent_spaces: parents,
        space_children: children,
    }
}

fn build_workspace() -> (Vector<RoomInfo>, Vec<RoomInfo>, String) {
    let mut all_rooms = Vector::new();
    let mut snapshot = Vec::new();
    let root_space_id = "!space-0:example.org".to_string();
    let child_space_id = "!space-1:example.org".to_string();

    let root_children = (0..120)
        .map(|ix| format!("!room-{ix}:example.org"))
        .chain(std::iter::once(child_space_id.clone()))
        .collect::<Vec<_>>();
    let child_children = (120..240)
        .map(|ix| format!("!room-{ix}:example.org"))
        .collect::<Vec<_>>();

    for item in [
        space(0, Vec::new(), root_children),
        space(1, vec![root_space_id.clone()], child_children),
    ] {
        all_rooms.push_back(item.clone());
        snapshot.push(item);
    }

    for ix in 0..240 {
        let parents = if ix < 120 {
            vec![root_space_id.clone()]
        } else {
            vec![child_space_id.clone()]
        };
        let item = room(ix, parents);
        all_rooms.push_back(item.clone());
        snapshot.push(item);
    }

    for ix in 0..400 {
        let item = dm(ix);
        all_rooms.push_back(item.clone());
        snapshot.push(item);
    }

    for ix in 240..720 {
        let item = room(ix, Vec::new());
        all_rooms.push_back(item.clone());
        snapshot.push(item);
    }

    (all_rooms, snapshot, root_space_id)
}

fn bench_room_hierarchy(c: &mut Criterion) {
    let (all_rooms, snapshot, root_space_id) = build_workspace();
    let mut group = c.benchmark_group("room_hierarchy");

    for (label, selection, show_rooms_in_home) in [
        ("home", RailSelection::Home, true),
        ("other_rooms", RailSelection::Rooms, false),
        ("space", RailSelection::Space(root_space_id.clone()), false),
    ] {
        group.bench_with_input(
            BenchmarkId::new("derive_room_lists", label),
            &(selection, show_rooms_in_home),
            |b, (selection, show_rooms_in_home)| {
                b.iter(|| derive_room_lists(&all_rooms, selection, *show_rooms_in_home));
            },
        );
    }

    let derived = derive_room_lists(
        &all_rooms,
        &RailSelection::Space(root_space_id.clone()),
        false,
    );
    group.bench_function("build_room_sections_space", |b| {
        b.iter(|| {
            build_room_sections(
                &RailSelection::Space(root_space_id.clone()),
                &derived.rooms,
                &snapshot,
            )
        });
    });

    group.finish();
}

criterion_group!(benches, bench_room_hierarchy);
criterion_main!(benches);
