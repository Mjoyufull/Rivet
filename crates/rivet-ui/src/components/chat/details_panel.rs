use crate::components::remote_image::{RemoteImage, avatar_fallback_label};
use crate::models::appearance::{avatar_radius_for, element_radius_small};
use crate::rooms::resolve_direct_room_profile;
use crate::theme::onedark::OneDarkThemeExt;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement;
use gpui_component::{Disableable, IconName, Sizable, StyledExt};
use matrix_sdk::deserialized_responses::SyncOrStrippedState;
use matrix_sdk::room::{RoomMember, RoomMemberRole};
use matrix_sdk::ruma::events::SyncStateEvent;
use matrix_sdk::ruma::events::room::history_visibility::{
    HistoryVisibility, RoomHistoryVisibilityEventContent,
};
use matrix_sdk::ruma::events::room::pinned_events::RoomPinnedEventsEventContent;
use matrix_sdk::ruma::events::room::topic::RoomTopicEventContent;
use matrix_sdk::{Room as MatrixRoom, RoomDisplayName, RoomMemberships};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MemberRoleBucket {
    Admin,
    Moderator,
    Member,
}

#[derive(Clone, Debug)]
struct MemberEntry {
    user_id: String,
    display_name: String,
    avatar_url: Option<String>,
    role: MemberRoleBucket,
}

#[derive(Clone, Debug)]
struct MemberGroup {
    label: &'static str,
    members: Vec<MemberEntry>,
}

#[derive(Clone, Debug)]
struct DirectRoomInfo {
    room_name: String,
    room_avatar_url: Option<String>,
    partner_display_name: String,
    partner_user_id: String,
    partner_avatar_url: Option<String>,
    topic: Option<String>,
    history_visibility: Option<String>,
    pinned_count: usize,
    member_count: usize,
    encrypted: bool,
}

#[derive(Clone, Debug)]
struct RoomMemberInfo {
    room_name: String,
    member_count: usize,
    groups: Vec<MemberGroup>,
}

#[derive(Clone, Debug)]
enum PanelMode {
    Members(RoomMemberInfo),
    Direct(DirectRoomInfo),
}

pub struct RoomDetailsPanel {
    room: MatrixRoom,
    loading: bool,
    mode: Option<PanelMode>,
    search_input: Entity<InputState>,
}

impl RoomDetailsPanel {
    pub fn new(room: MatrixRoom, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search members..."));
        cx.subscribe(&search_input, |_, _, _: &InputEvent, cx| cx.notify())
            .detach();

        let panel = Self {
            room,
            loading: true,
            mode: None,
            search_input,
        };

        panel.spawn_refresh(cx);
        panel
    }

    fn spawn_refresh(&self, cx: &mut Context<Self>) {
        let room = self.room.clone();
        let panel = cx.entity().downgrade();
        let mut async_cx = cx.to_async();

        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                let mode = load_panel_mode(room).await;
                let _ = panel.update(&mut async_cx, |this, cx| {
                    this.loading = false;
                    this.mode = Some(mode);
                    cx.notify();
                });
            })
            .detach();
    }

    fn render_avatar_tile(
        avatar_url: Option<&String>,
        size: Pixels,
        label: &str,
        fallback_source: &str,
        cx: &App,
    ) -> AnyElement {
        let fallback = avatar_fallback_label(label, fallback_source);
        if let Some(url) = avatar_url {
            RemoteImage::new(url.clone())
                .size(size)
                .avatar()
                .fallback_text(fallback)
                .into_any_element()
        } else {
            let theme = cx.onedark_theme();
            div()
                .size(size)
                .corner_radii(Corners::all(avatar_radius_for(size, cx)))
                .bg(theme.accent.opacity(0.18))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(theme.accent)
                        .font_weight(FontWeight::BOLD)
                        .child(fallback),
                )
                .into_any_element()
        }
    }

    fn render_members_panel(
        &self,
        info: &RoomMemberInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let query = self.search_input.read(cx).text().to_string().to_lowercase();
        let card_radius = element_radius_small(cx);

        let filtered_groups: Vec<_> = info
            .groups
            .iter()
            .filter_map(|group| {
                let members = group
                    .members
                    .iter()
                    .filter(|member| {
                        query.is_empty()
                            || member.display_name.to_lowercase().contains(&query)
                            || member.user_id.to_lowercase().contains(&query)
                    })
                    .cloned()
                    .collect::<Vec<_>>();

                if members.is_empty() {
                    None
                } else {
                    Some((group.label, members))
                }
            })
            .collect();

        div()
            .w(px(300.0))
            .min_w(px(260.0))
            .max_w(px(340.0))
            .h_full()
            .flex_shrink_0()
            .bg(theme.sidebar_background)
            .border_l(px(1.0))
            .border_color(theme.border)
            .flex()
            .flex_col()
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b(px(1.0))
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text)
                            .child(format!("{} Members", info.member_count)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .truncate()
                            .child(info.room_name.clone()),
                    ),
            )
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b(px(1.0))
                    .border_color(theme.border.opacity(0.65))
                    .child(Input::new(&self.search_input).prefix(IconName::Search)),
            )
            .child(div().flex_1().overflow_y_scrollbar().child(
                div().px_3().py_4().flex().flex_col().gap_5().children(
                    if filtered_groups.is_empty() {
                        vec![
                            div()
                                .px_1()
                                .py_2()
                                .text_sm()
                                .text_color(theme.text_muted)
                                .child("No matching members")
                                .into_any_element(),
                        ]
                    } else {
                        filtered_groups
                            .into_iter()
                            .map(|(label, members)| {
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .px_1()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.text_muted)
                                            .child(format!("{label} — {}", members.len())),
                                    )
                                    .children(members.into_iter().map(|member| {
                                        div()
                                            .px_2()
                                            .py_2()
                                            .corner_radii(Corners::all(card_radius))
                                            .hover(|style| style.bg(theme.sidebar_item_hover))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(Self::render_avatar_tile(
                                                        member.avatar_url.as_ref(),
                                                        px(36.0),
                                                        &member.display_name,
                                                        &member.user_id,
                                                        cx,
                                                    ))
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(
                                                                        FontWeight::SEMIBOLD,
                                                                    )
                                                                    .text_color(theme.text)
                                                                    .truncate()
                                                                    .child(
                                                                        member.display_name.clone(),
                                                                    ),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(theme.text_muted)
                                                                    .truncate()
                                                                    .child(member.user_id),
                                                            ),
                                                    ),
                                            )
                                            .into_any_element()
                                    }))
                            })
                            .map(|section| section.into_any_element())
                            .collect::<Vec<_>>()
                    },
                ),
            ))
    }

    fn render_badge(&self, text: &str, accent: impl Into<Hsla>, cx: &App) -> AnyElement {
        let accent = accent.into();
        let radius = px((f32::from(element_radius_small(cx)) * 0.85).clamp(6.0, 16.0));
        div()
            .px_2()
            .py_1()
            .corner_radii(Corners::all(radius))
            .bg(accent.opacity(0.18))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(accent)
            .child(text.to_string())
            .into_any_element()
    }

    fn render_info_row(
        &self,
        icon: IconName,
        label: &str,
        value: impl Into<String>,
        cx: &App,
    ) -> AnyElement {
        let theme = cx.onedark_theme();
        let radius = element_radius_small(cx);
        div()
            .px_3()
            .py_2()
            .corner_radii(Corners::all(radius))
            .hover(|style| style.bg(theme.sidebar_item_hover))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                gpui_component::Icon::new(icon)
                                    .with_size(gpui_component::Size::Small)
                                    .text_color(theme.text_muted),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.text)
                                    .child(label.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child(value.into()),
                    ),
            )
            .into_any_element()
    }

    fn render_direct_panel(
        &self,
        info: &DirectRoomInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.onedark_theme();

        div()
            .w(px(300.0))
            .min_w(px(260.0))
            .max_w(px(340.0))
            .h_full()
            .flex_shrink_0()
            .bg(theme.sidebar_background)
            .border_l(px(1.0))
            .border_color(theme.border)
            .flex()
            .flex_col()
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b(px(1.0))
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text)
                            .child("Room Info"),
                    )
                    .child(
                        Button::new("room-info-indicator")
                            .icon(IconName::PanelRight)
                            .ghost()
                            .disabled(true),
                    ),
            )
            .child(
                div().flex_1().overflow_y_scrollbar().child(
                    div()
                        .px_4()
                        .py_5()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_5()
                        .child(Self::render_avatar_tile(
                            info.room_avatar_url
                                .as_ref()
                                .or_else(|| info.partner_avatar_url.as_ref()),
                            px(84.0),
                            &info.partner_display_name,
                            &info.partner_user_id,
                            cx,
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xl()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text)
                                        .child(info.room_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.text_muted)
                                        .child(info.partner_user_id.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .justify_center()
                                .gap_2()
                                .child(if info.encrypted {
                                    self.render_badge("Encrypted", rgb(0x56b6c2), cx)
                                } else {
                                    self.render_badge("Unencrypted", rgb(0xe06c75), cx)
                                })
                                .children(
                                    info.history_visibility
                                        .as_ref()
                                        .map(|label| {
                                            vec![self.render_badge(label, theme.accent, cx)]
                                        })
                                        .unwrap_or_default(),
                                ),
                        )
                        .child(div().w_full().h_px().bg(theme.border.opacity(0.7)))
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.text_muted)
                                        .child("TOPIC"),
                                )
                                .child(div().text_sm().text_color(theme.text).child(
                                    info.topic.clone().unwrap_or_else(|| {
                                        "No topic has been set for this conversation yet."
                                            .to_string()
                                    }),
                                )),
                        )
                        .child(div().w_full().h_px().bg(theme.border.opacity(0.7)))
                        .child(div().w_full().flex().flex_col().gap_1().children(vec![
                            self.render_info_row(
                                IconName::User,
                                "People",
                                info.member_count.to_string(),
                                cx,
                            ),
                            self.render_info_row(
                                IconName::Star,
                                "Pinned messages",
                                info.pinned_count.to_string(),
                                cx,
                            ),
                            self.render_info_row(
                                IconName::Info,
                                "Room ID",
                                self.room.room_id().to_string(),
                                cx,
                            ),
                        ])),
                ),
            )
    }
}

impl Render for RoomDetailsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();

        if self.loading {
            return div()
                .w(px(300.0))
                .min_w(px(260.0))
                .max_w(px(340.0))
                .h_full()
                .flex_shrink_0()
                .bg(theme.sidebar_background)
                .border_l(px(1.0))
                .border_color(theme.border)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child("Loading room details..."),
                )
                .into_any_element();
        }

        match self.mode.as_ref() {
            Some(PanelMode::Members(info)) => {
                self.render_members_panel(info, cx).into_any_element()
            }
            Some(PanelMode::Direct(info)) => self.render_direct_panel(info, cx).into_any_element(),
            None => div()
                .w(px(300.0))
                .min_w(px(260.0))
                .max_w(px(340.0))
                .h_full()
                .flex_shrink_0()
                .bg(theme.sidebar_background)
                .border_l(px(1.0))
                .border_color(theme.border)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child("No room details available"),
                )
                .into_any_element(),
        }
    }
}

fn member_display_name(member: &RoomMember) -> String {
    member
        .display_name()
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| member.user_id().localpart().to_string())
}

fn bucket_for_role(role: RoomMemberRole) -> MemberRoleBucket {
    match role {
        RoomMemberRole::Creator | RoomMemberRole::Administrator => MemberRoleBucket::Admin,
        RoomMemberRole::Moderator => MemberRoleBucket::Moderator,
        RoomMemberRole::User => MemberRoleBucket::Member,
    }
}

fn room_display_name_or_id(
    room: &MatrixRoom,
    display_name: Result<RoomDisplayName, impl std::fmt::Debug>,
) -> String {
    match display_name {
        Ok(RoomDisplayName::Named(name))
        | Ok(RoomDisplayName::Calculated(name))
        | Ok(RoomDisplayName::Aliased(name)) => name,
        Ok(RoomDisplayName::EmptyWas(name)) => format!("Empty Room (was {name})"),
        Ok(RoomDisplayName::Empty) | Err(_) => room.room_id().to_string(),
    }
}

fn extract_topic(
    event: Option<
        matrix_sdk::deserialized_responses::RawSyncOrStrippedState<RoomTopicEventContent>,
    >,
) -> Option<String> {
    let event = event?;
    match event.deserialize().ok()? {
        SyncOrStrippedState::Sync(SyncStateEvent::Original(ev)) => {
            let topic = ev.content.topic.trim().to_string();
            (!topic.is_empty()).then_some(topic)
        }
        SyncOrStrippedState::Stripped(ev) => {
            let topic = ev.content.topic?.trim().to_string();
            (!topic.is_empty()).then_some(topic)
        }
        _ => None,
    }
}

fn extract_history_visibility(
    event: Option<
        matrix_sdk::deserialized_responses::RawSyncOrStrippedState<
            RoomHistoryVisibilityEventContent,
        >,
    >,
) -> Option<String> {
    let event = event?;
    let visibility = match event.deserialize().ok()? {
        SyncOrStrippedState::Sync(ev) => ev.history_visibility().clone(),
        SyncOrStrippedState::Stripped(ev) => ev.content.history_visibility,
    };

    Some(match visibility {
        HistoryVisibility::Invited => "Invited members see history".to_string(),
        HistoryVisibility::Joined => "New members see history".to_string(),
        HistoryVisibility::Shared => "Members can read full history".to_string(),
        HistoryVisibility::WorldReadable => "World readable".to_string(),
        HistoryVisibility::_Custom(_) => "Custom history visibility".to_string(),
        _ => "History visibility configured".to_string(),
    })
}

fn extract_pinned_count(
    event: Option<
        matrix_sdk::deserialized_responses::RawSyncOrStrippedState<RoomPinnedEventsEventContent>,
    >,
) -> usize {
    let Some(event) = event else {
        return 0;
    };

    match event.deserialize().ok() {
        Some(SyncOrStrippedState::Sync(SyncStateEvent::Original(ev))) => ev.content.pinned.len(),
        Some(SyncOrStrippedState::Stripped(ev)) => {
            ev.content.pinned.map(|pinned| pinned.len()).unwrap_or(0)
        }
        _ => 0,
    }
}

async fn load_panel_mode(room: MatrixRoom) -> PanelMode {
    let is_direct = room.is_direct().await.unwrap_or(false);
    let display_name = room.display_name().await;
    let room_name = room_display_name_or_id(&room, display_name);
    let encrypted = room
        .latest_encryption_state()
        .await
        .map(|state| state.is_encrypted())
        .unwrap_or_else(|_| room.encryption_state().is_encrypted());
    let topic = extract_topic(
        room.get_state_event_static::<RoomTopicEventContent>()
            .await
            .ok()
            .flatten(),
    );
    let history_visibility = extract_history_visibility(
        room.get_state_event_static::<RoomHistoryVisibilityEventContent>()
            .await
            .ok()
            .flatten(),
    );
    let pinned_count = extract_pinned_count(
        room.get_state_event_static::<RoomPinnedEventsEventContent>()
            .await
            .ok()
            .flatten(),
    );
    let member_count = room.active_members_count() as usize;

    let mut members = match room.members_no_sync(RoomMemberships::ACTIVE).await {
        Ok(members) => members,
        Err(_) => room
            .members(RoomMemberships::ACTIVE)
            .await
            .unwrap_or_default(),
    };

    members.sort_by(|a, b| {
        bucket_for_role(a.suggested_role_for_power_level())
            .cmp(&bucket_for_role(b.suggested_role_for_power_level()))
            .then_with(|| {
                member_display_name(a)
                    .to_lowercase()
                    .cmp(&member_display_name(b).to_lowercase())
            })
    });

    if is_direct {
        let direct_profile = resolve_direct_room_profile(&room).await;
        let own_user_id = room.own_user_id().to_owned();
        let partner = members
            .iter()
            .find(|member| member.user_id() != own_user_id)
            .cloned()
            .or_else(|| members.first().cloned());

        let partner_display_name = direct_profile
            .as_ref()
            .map(|profile| profile.display_name.clone())
            .or_else(|| partner.as_ref().map(member_display_name))
            .unwrap_or_else(|| room_name.clone());
        let partner_user_id = direct_profile
            .as_ref()
            .map(|profile| profile.user_id.clone())
            .or_else(|| partner.as_ref().map(|member| member.user_id().to_string()))
            .unwrap_or_else(|| room.room_id().to_string());
        let partner_avatar_url = direct_profile
            .as_ref()
            .and_then(|profile| profile.avatar_url.clone())
            .or_else(|| {
                partner
                    .as_ref()
                    .and_then(|member| member.avatar_url().map(|url| url.to_string()))
            });

        PanelMode::Direct(DirectRoomInfo {
            room_name,
            room_avatar_url: room.avatar_url().map(|url| url.to_string()),
            partner_display_name,
            partner_user_id,
            partner_avatar_url,
            topic,
            history_visibility,
            pinned_count,
            member_count,
            encrypted,
        })
    } else {
        let mut admins = Vec::new();
        let mut moderators = Vec::new();
        let mut members_group = Vec::new();

        for member in members {
            let entry = MemberEntry {
                user_id: member.user_id().to_string(),
                display_name: member_display_name(&member),
                avatar_url: member.avatar_url().map(|url| url.to_string()),
                role: bucket_for_role(member.suggested_role_for_power_level()),
            };

            match entry.role {
                MemberRoleBucket::Admin => admins.push(entry),
                MemberRoleBucket::Moderator => moderators.push(entry),
                MemberRoleBucket::Member => members_group.push(entry),
            }
        }

        let mut groups = Vec::new();
        if !admins.is_empty() {
            groups.push(MemberGroup {
                label: "Admin",
                members: admins,
            });
        }
        if !moderators.is_empty() {
            groups.push(MemberGroup {
                label: "Moderator",
                members: moderators,
            });
        }
        if !members_group.is_empty() {
            groups.push(MemberGroup {
                label: "Members",
                members: members_group,
            });
        }

        let _ = topic;
        let _ = history_visibility;
        let _ = pinned_count;
        let _ = encrypted;

        PanelMode::Members(RoomMemberInfo {
            room_name,
            member_count,
            groups,
        })
    }
}
