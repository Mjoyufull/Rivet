use matrix_sdk::Room as MatrixRoom;
use matrix_sdk::RoomMemberships;
use matrix_sdk::room::RoomMember;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct DirectRoomProfile {
    pub user_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

pub async fn resolve_direct_room_profile(room: &MatrixRoom) -> Option<DirectRoomProfile> {
    let own_user_id = room.own_user_id().to_string();
    let direct_targets = room
        .direct_targets()
        .into_iter()
        .map(|target| target.to_string())
        .collect::<HashSet<_>>();

    let memberships = RoomMemberships::ACTIVE;
    let local_members = room.members_no_sync(memberships).await.ok();
    let members = if room.are_members_synced() {
        local_members.unwrap_or_default()
    } else {
        let _ = room.sync_members().await;
        room.members(memberships)
            .await
            .ok()
            .or(local_members)
            .unwrap_or_default()
    };

    let counterpart = select_direct_counterpart(&members, &own_user_id, &direct_targets)?;

    Some(DirectRoomProfile {
        user_id: counterpart.user_id().to_string(),
        display_name: member_display_name(counterpart),
        avatar_url: counterpart.avatar_url().map(|url| url.to_string()),
    })
}

fn select_direct_counterpart<'a>(
    members: &'a [RoomMember],
    own_user_id: &str,
    direct_targets: &HashSet<String>,
) -> Option<&'a RoomMember> {
    let other_members = members
        .iter()
        .filter(|member| member.user_id().as_str() != own_user_id)
        .collect::<Vec<_>>();

    other_members
        .iter()
        .copied()
        .find(|member| direct_targets.contains(member.user_id().as_str()))
        .or_else(|| other_members.first().copied())
}

fn member_display_name(member: &RoomMember) -> String {
    member
        .display_name()
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| member.user_id().localpart().to_string())
}
