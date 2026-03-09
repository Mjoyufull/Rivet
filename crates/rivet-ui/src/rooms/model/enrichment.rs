use super::RoomInfo;
use crate::rooms::resolve_direct_room_profile;
use futures::StreamExt;
use matrix_sdk::RoomDisplayName;
use matrix_sdk::RoomState;
use matrix_sdk::deserialized_responses::SyncOrStrippedState;
use matrix_sdk::room::ParentSpace;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::SyncStateEvent;
use matrix_sdk::ruma::events::space::child::SpaceChildEventContent;
use matrix_sdk_ui::room_list_service::RoomListItem;

pub(super) async fn process_room_items(items: Vec<RoomListItem>) -> Vec<RoomInfo> {
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

        if is_direct {
            let needs_direct_profile = avatar_url.is_none()
                || name.starts_with('@')
                || name == room_id_str
                || name.starts_with("Empty Room");

            if needs_direct_profile && let Some(profile) = resolve_direct_room_profile(&room).await
            {
                if avatar_url.is_none() {
                    avatar_url = profile.avatar_url;
                }

                if name.starts_with('@') || name == room_id_str || name.starts_with("Empty Room") {
                    name = profile.display_name;
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
