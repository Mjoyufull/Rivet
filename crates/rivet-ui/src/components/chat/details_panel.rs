use crate::components::remote_image::{RemoteImage, avatar_fallback_label};
use crate::models::appearance::{avatar_radius_for, element_radius_small};
use crate::models::image_cache::ImagePriority;
use crate::perf;
use crate::rooms::{DirectRoomProfile, resolve_direct_room_profile_cached};
use crate::theme::onedark::OneDarkThemeExt;
use crate::timeline::TimelineView;
use chrono::{DateTime, Local};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement;
use gpui_component::{Icon, IconName, Sizable, StyledExt};
use matrix_sdk::deserialized_responses::{SyncOrStrippedState, TimelineEvent};
use matrix_sdk::room::{RoomMember, RoomMemberRole};
use matrix_sdk::ruma::events::room::history_visibility::{
    HistoryVisibility, RoomHistoryVisibilityEventContent,
};
use matrix_sdk::ruma::events::room::message::MessageType;
use matrix_sdk::ruma::events::room::topic::RoomTopicEventContent;
use matrix_sdk::ruma::events::{AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncStateEvent};
use matrix_sdk::{Room as MatrixRoom, RoomDisplayName, RoomMemberships};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

const MEMBER_LIST_SYNC_THRESHOLD: u64 = 128;
const MEMBER_PANEL_AUTOLOAD_THRESHOLD: u64 = 128;
const MEMBER_LIST_ROW_HEIGHT: Pixels = px(52.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetailsTopBarMode {
    RoomInfo,
    Members,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DetailsPanelState {
    mode: DetailsTopBarMode,
    visible: bool,
}

#[derive(Clone)]
pub struct DetailsPanelPreferences {
    room_state: Rc<Cell<DetailsPanelState>>,
    dm_state: Rc<Cell<DetailsPanelState>>,
    cached_details: Rc<RefCell<HashMap<String, CachedRoomDetails>>>,
}

impl Default for DetailsPanelPreferences {
    fn default() -> Self {
        Self {
            room_state: Rc::new(Cell::new(DetailsPanelState {
                mode: DetailsTopBarMode::Members,
                visible: true,
            })),
            dm_state: Rc::new(Cell::new(DetailsPanelState {
                mode: DetailsTopBarMode::RoomInfo,
                visible: true,
            })),
            cached_details: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}

impl DetailsPanelPreferences {
    fn state_for_room(&self, is_direct: bool) -> DetailsPanelState {
        if is_direct {
            self.dm_state.get()
        } else {
            self.room_state.get()
        }
    }

    fn set_state_for_room(&self, is_direct: bool, state: DetailsPanelState) {
        if is_direct {
            self.dm_state.set(state);
        } else {
            self.room_state.set(state);
        }
    }

    fn cached_room_info(&self, room_id: &str) -> Option<RoomInfoData> {
        self.cached_details
            .borrow()
            .get(room_id)
            .and_then(|entry| entry.room_info.clone())
    }

    fn cache_room_info(&self, room_id: &str, room_info: RoomInfoData) {
        self.cached_details
            .borrow_mut()
            .entry(room_id.to_string())
            .or_default()
            .room_info = Some(room_info);
    }

    fn cached_members(&self, room_id: &str) -> Option<MembersData> {
        self.cached_details
            .borrow()
            .get(room_id)
            .and_then(|entry| entry.members.clone())
    }

    fn cache_members(&self, room_id: &str, members: MembersData) {
        self.cached_details
            .borrow_mut()
            .entry(room_id.to_string())
            .or_default()
            .members = Some(members);
    }

    fn cached_pinned_messages(&self, room_id: &str) -> Option<Vec<PinnedMessageEntry>> {
        self.cached_details
            .borrow()
            .get(room_id)
            .and_then(|entry| entry.pinned_messages.clone())
    }

    fn cache_pinned_messages(&self, room_id: &str, messages: Vec<PinnedMessageEntry>) {
        self.cached_details
            .borrow_mut()
            .entry(room_id.to_string())
            .or_default()
            .pinned_messages = Some(messages);
    }
}

#[derive(Clone, Debug, Default)]
struct CachedRoomDetails {
    room_info: Option<RoomInfoData>,
    members: Option<MembersData>,
    pinned_messages: Option<Vec<PinnedMessageEntry>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MemberRoleBucket {
    Admin,
    Moderator,
    Member,
}

#[derive(Clone, Debug)]
struct MemberEntry {
    user_id: String,
    user_id_lower: String,
    display_name: String,
    display_name_lower: String,
    avatar_url: Option<String>,
    role: MemberRoleBucket,
}

#[derive(Clone, Debug)]
struct MemberGroup {
    label: &'static str,
    members: Vec<MemberEntry>,
}

#[derive(Clone, Debug)]
struct PinnedMessageEntry {
    event_id: String,
    sender_id: String,
    sender_name: String,
    avatar_url: Option<String>,
    body: String,
    timestamp: String,
}

#[derive(Clone, Debug)]
struct RoomInfoData {
    room_name: String,
    room_avatar_url: Option<String>,
    hero_label: String,
    subtitle: Option<String>,
    topic: Option<String>,
    history_visibility: Option<String>,
    pinned_count: usize,
    member_count: usize,
    encrypted: bool,
    room_id: String,
}

#[derive(Clone, Debug)]
struct MembersData {
    groups: Vec<MemberGroup>,
    member_lookup: HashMap<String, MemberEntry>,
    synced: bool,
}

#[derive(Clone, Debug)]
enum AsyncSection<T> {
    Idle,
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> Default for AsyncSection<T> {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Clone, Debug)]
enum MemberListRow {
    Header(String),
    Member(MemberEntry),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanelView {
    RoomInfo,
    Members,
    PinnedMessages,
}

pub struct RoomDetailsPanel {
    room: MatrixRoom,
    room_id: String,
    is_direct: bool,
    room_info: AsyncSection<RoomInfoData>,
    members: AsyncSection<MembersData>,
    pinned_messages: AsyncSection<Vec<PinnedMessageEntry>>,
    active_view: PanelView,
    visible: bool,
    preferences: DetailsPanelPreferences,
    timeline_view: Entity<TimelineView>,
    search_input: Entity<InputState>,
    members_scroll_handle: UniformListScrollHandle,
    members_revision: u64,
    member_rows_revision: u64,
    member_rows_query: String,
    member_rows: Rc<Vec<MemberListRow>>,
}

impl RoomDetailsPanel {
    pub fn new(
        room: MatrixRoom,
        is_direct: bool,
        preferences: DetailsPanelPreferences,
        timeline_view: Entity<TimelineView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search members..."));
        cx.subscribe(&search_input, |_, _, _: &InputEvent, cx| cx.notify())
            .detach();
        let panel_state = preferences.state_for_room(is_direct);
        let room_id = room.room_id().to_string();
        let cached_room_info = preferences.cached_room_info(&room_id);
        let cached_members = preferences.cached_members(&room_id);
        let cached_pinned_messages = preferences.cached_pinned_messages(&room_id);
        let members_revision = u64::from(cached_members.is_some());
        let should_defer_member_autoload = !is_direct
            && cached_members.is_none()
            && room.active_members_count() > MEMBER_PANEL_AUTOLOAD_THRESHOLD;
        let active_view = match panel_state.mode {
            DetailsTopBarMode::RoomInfo => PanelView::RoomInfo,
            DetailsTopBarMode::Members if should_defer_member_autoload => PanelView::RoomInfo,
            DetailsTopBarMode::Members => PanelView::Members,
        };

        let mut panel = Self {
            room,
            room_id,
            is_direct,
            room_info: cached_room_info.map_or(AsyncSection::Loading, AsyncSection::Ready),
            members: cached_members.map_or(AsyncSection::Idle, AsyncSection::Ready),
            pinned_messages: cached_pinned_messages.map_or(AsyncSection::Idle, AsyncSection::Ready),
            active_view,
            visible: panel_state.visible,
            preferences,
            timeline_view,
            search_input,
            members_scroll_handle: UniformListScrollHandle::new(),
            members_revision,
            member_rows_revision: u64::MAX,
            member_rows_query: String::new(),
            member_rows: Rc::new(Vec::new()),
        };

        if matches!(panel.room_info, AsyncSection::Loading) {
            panel.spawn_room_info_refresh(cx);
        }
        panel.ensure_active_view_loaded(cx);
        panel
    }

    pub fn show_room_info(&mut self, cx: &mut Context<Self>) {
        self.active_view = PanelView::RoomInfo;
        self.visible = true;
        self.persist_panel_state();
        self.ensure_active_view_loaded(cx);
        cx.notify();
    }

    pub fn toggle_room_info(&mut self, cx: &mut Context<Self>) {
        if self.visible
            && matches!(
                self.active_view,
                PanelView::RoomInfo | PanelView::PinnedMessages
            )
        {
            self.visible = false;
        } else {
            self.active_view = PanelView::RoomInfo;
            self.visible = true;
        }
        self.persist_panel_state();
        self.ensure_active_view_loaded(cx);
        cx.notify();
    }

    pub fn toggle_members(&mut self, cx: &mut Context<Self>) {
        if self.visible && matches!(self.active_view, PanelView::Members) {
            self.visible = false;
        } else {
            self.active_view = PanelView::Members;
            self.visible = true;
        }
        self.persist_panel_state();
        self.ensure_active_view_loaded(cx);
        cx.notify();
    }

    pub fn top_bar_mode(&self) -> DetailsTopBarMode {
        match self.active_view {
            PanelView::Members => DetailsTopBarMode::Members,
            PanelView::RoomInfo | PanelView::PinnedMessages => DetailsTopBarMode::RoomInfo,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    fn persist_panel_state(&self) {
        self.preferences.set_state_for_room(
            self.is_direct,
            DetailsPanelState {
                mode: self.top_bar_mode(),
                visible: self.visible,
            },
        );
    }

    fn ensure_active_view_loaded(&mut self, cx: &mut Context<Self>) {
        if !self.visible {
            return;
        }

        match self.active_view {
            PanelView::RoomInfo => {
                if matches!(self.room_info, AsyncSection::Idle) {
                    self.room_info = AsyncSection::Loading;
                    self.spawn_room_info_refresh(cx);
                }
            }
            PanelView::Members => {
                if matches!(self.members, AsyncSection::Idle) {
                    self.members = AsyncSection::Loading;
                    self.spawn_members_refresh(cx);
                }
            }
            PanelView::PinnedMessages => {
                if matches!(self.pinned_messages, AsyncSection::Idle) {
                    self.pinned_messages = AsyncSection::Loading;
                    self.spawn_pinned_messages_refresh(cx);
                }
            }
        }
    }

    fn spawn_room_info_refresh(&self, cx: &mut Context<Self>) {
        let room = self.room.clone();
        let panel = cx.entity().downgrade();
        let mut async_cx = cx.to_async();
        let room_id = self.room_id.clone();
        let preferences = self.preferences.clone();

        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                let started = Instant::now();
                let details = load_room_info(room).await;
                perf::log_if_slow(
                    "details.room_info.load",
                    started,
                    Duration::from_millis(24),
                    || {
                        format!(
                            "room={} pinned_count={} member_count={}",
                            details.room_id, details.pinned_count, details.member_count
                        )
                    },
                );
                let _ = panel.update(&mut async_cx, |this, cx| {
                    preferences.cache_room_info(&room_id, details.clone());
                    this.room_info = AsyncSection::Ready(details);
                    cx.notify();
                });
            })
            .detach();
    }

    fn spawn_members_refresh(&self, cx: &mut Context<Self>) {
        let room = self.room.clone();
        let panel = cx.entity().downgrade();
        let mut async_cx = cx.to_async();
        let room_id = self.room_id.clone();
        let preferences = self.preferences.clone();

        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                let started = Instant::now();
                let result = load_members_data(room).await;
                perf::log_if_slow(
                    "details.members.load",
                    started,
                    Duration::from_millis(40),
                    || match &result {
                        Ok(members) => format!(
                            "groups={} members={} synced={}",
                            members.groups.len(),
                            members.member_lookup.len(),
                            members.synced
                        ),
                        Err(error) => format!("error={error}"),
                    },
                );
                let _ = panel.update(&mut async_cx, |this, cx| {
                    match result {
                        Ok(members) => {
                            preferences.cache_members(&room_id, members.clone());
                            this.members = AsyncSection::Ready(members);
                            this.members_revision = this.members_revision.wrapping_add(1);
                        }
                        Err(error) => this.members = AsyncSection::Failed(error),
                    }
                    cx.notify();
                });
            })
            .detach();
    }

    fn spawn_pinned_messages_refresh(&self, cx: &mut Context<Self>) {
        let room = self.room.clone();
        let member_lookup = match &self.members {
            AsyncSection::Ready(members) => Some(members.member_lookup.clone()),
            _ => None,
        };
        let panel = cx.entity().downgrade();
        let mut async_cx = cx.to_async();
        let room_id = self.room_id.clone();
        let preferences = self.preferences.clone();

        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                let started = Instant::now();
                let result = load_pinned_messages_data(room, member_lookup).await;
                perf::log_if_slow(
                    "details.pinned.load",
                    started,
                    Duration::from_millis(24),
                    || match &result {
                        Ok(messages) => format!("count={}", messages.len()),
                        Err(error) => format!("error={error}"),
                    },
                );
                let _ = panel.update(&mut async_cx, |this, cx| {
                    match result {
                        Ok(messages) => {
                            preferences.cache_pinned_messages(&room_id, messages.clone());
                            this.pinned_messages = AsyncSection::Ready(messages);
                        }
                        Err(error) => this.pinned_messages = AsyncSection::Failed(error),
                    }
                    cx.notify();
                });
            })
            .detach();
    }

    fn member_rows_for_query(&mut self, query: &str) -> Rc<Vec<MemberListRow>> {
        if self.member_rows_revision == self.members_revision && self.member_rows_query == query {
            return self.member_rows.clone();
        }

        let started = Instant::now();
        self.member_rows = match &self.members {
            AsyncSection::Ready(members) => Rc::new(build_member_list_rows(&members.groups, query)),
            _ => Rc::new(Vec::new()),
        };
        self.member_rows_query = query.to_string();
        self.member_rows_revision = self.members_revision;
        let row_count = self.member_rows.len();
        perf::log_if_slow(
            "details.members.rows",
            started,
            Duration::from_millis(8),
            || {
                format!(
                    "room={} query_len={} rows={row_count}",
                    self.room_id,
                    query.len()
                )
            },
        );
        self.member_rows.clone()
    }

    fn render_avatar_tile(
        avatar_url: Option<&String>,
        size: Pixels,
        label: &str,
        fallback_source: &str,
        priority: ImagePriority,
        cx: &App,
    ) -> AnyElement {
        let fallback = avatar_fallback_label(label, fallback_source);
        if let Some(url) = avatar_url {
            let image = match priority {
                ImagePriority::High => RemoteImage::new(url.clone()).high_priority(),
                ImagePriority::Low => RemoteImage::new(url.clone()).low_priority(),
                ImagePriority::Normal => RemoteImage::new(url.clone()),
            };
            image
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

    fn render_static_info_row(
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
                                Icon::new(icon)
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
                            .truncate()
                            .child(value.into()),
                    ),
            )
            .into_any_element()
    }

    fn render_nav_row(
        &self,
        icon: IconName,
        label: &str,
        value: impl Into<String>,
        next_view: PanelView,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.onedark_theme();
        let radius = element_radius_small(cx);
        let view = cx.entity().clone();

        div()
            .px_3()
            .py_2()
            .corner_radii(Corners::all(radius))
            .cursor_pointer()
            .hover(|style| style.bg(theme.sidebar_item_hover))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                let _ = view.update(cx, |this, cx| {
                    this.active_view = next_view;
                    this.visible = true;
                    this.persist_panel_state();
                    this.ensure_active_view_loaded(cx);
                    cx.notify();
                });
            })
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
                                Icon::new(icon)
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
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.text_muted)
                                    .child(value.into()),
                            )
                            .child(
                                Icon::new(IconName::ChevronRight)
                                    .with_size(gpui_component::Size::Small)
                                    .text_color(theme.text_muted),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_panel_frame(
        &self,
        title: impl Into<String>,
        subtitle: Option<String>,
        show_back: bool,
        body: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.onedark_theme();
        let view = cx.entity().clone();

        div()
            .size_full()
            .bg(theme.sidebar_background)
            .border_l(px(1.0))
            .border_color(theme.border)
            .flex()
            .flex_col()
            .min_h_0()
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b(px(1.0))
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(if show_back {
                                Button::new("details-panel-back")
                                    .icon(IconName::ChevronLeft)
                                    .ghost()
                                    .on_click(move |_, _, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            this.show_room_info(cx);
                                        });
                                    })
                                    .into_any_element()
                            } else {
                                div().w_0().into_any_element()
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme.text)
                                            .child(title.into()),
                                    )
                                    .children(subtitle.into_iter().map(|subtitle| {
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_muted)
                                            .child(subtitle)
                                            .into_any_element()
                                    })),
                            ),
                    ),
            )
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    fn render_room_info(&self, info: &RoomInfoData, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.onedark_theme();
        let body = div().size_full().min_h_0().overflow_y_scrollbar().child(
            div()
                .px_4()
                .py_5()
                .flex()
                .flex_col()
                .items_center()
                .gap_5()
                .child(Self::render_avatar_tile(
                    info.room_avatar_url.as_ref(),
                    px(84.0),
                    &info.hero_label,
                    &info.room_id,
                    ImagePriority::High,
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
                                .text_center()
                                .child(info.room_name.clone()),
                        )
                        .children(info.subtitle.iter().map(|subtitle| {
                            div()
                                .text_sm()
                                .text_color(theme.text_muted)
                                .text_center()
                                .child(subtitle.clone())
                                .into_any_element()
                        })),
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
                                .map(|label| vec![self.render_badge(label, theme.accent, cx)])
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
                                "No topic has been set for this conversation yet.".to_string()
                            }),
                        )),
                )
                .child(div().w_full().h_px().bg(theme.border.opacity(0.7)))
                .child(div().w_full().flex().flex_col().gap_1().children(vec![
                    self.render_nav_row(
                        IconName::User,
                        "People",
                        info.member_count.to_string(),
                        PanelView::Members,
                        cx,
                    ),
                    self.render_nav_row(
                        IconName::Star,
                        "Pinned messages",
                        info.pinned_count.to_string(),
                        PanelView::PinnedMessages,
                        cx,
                    ),
                    self.render_static_info_row(
                        IconName::Info,
                        "Room ID",
                        info.room_id.clone(),
                        cx,
                    ),
                ])),
        );

        self.render_panel_frame("Room Info", None, false, body.into_any_element(), cx)
    }

    fn render_members_panel(&mut self, info: &RoomInfoData, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.onedark_theme();
        let query = self.search_input.read(cx).text().to_string();

        let body = div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b(px(1.0))
                    .border_color(theme.border.opacity(0.65))
                    .child(Input::new(&self.search_input).prefix(IconName::Search)),
            )
            .child(
                match &self.members {
                    AsyncSection::Idle | AsyncSection::Loading => div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.text_muted)
                                .child("Loading members..."),
                        )
                        .into_any_element(),
                    AsyncSection::Failed(error) => div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .px_4()
                                .text_sm()
                                .text_color(theme.text_muted)
                                .text_center()
                                .child(error.clone()),
                        )
                        .into_any_element(),
                    AsyncSection::Ready(members) => {
                        let synced = members.synced;
                        let rows = self.member_rows_for_query(&query);
                        let scroll_handle = self.members_scroll_handle.clone();
                        let room_id = info.room_id.clone();
                        let rows_for_list = rows.clone();

                        div()
                            .flex_1()
                            .min_h_0()
                            .relative()
                            .when(!synced, |this| {
                                this.child(
                                    div()
                                        .px_4()
                                        .py_2()
                                        .border_b(px(1.0))
                                        .border_color(theme.border.opacity(0.45))
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child(format!(
                                            "Showing cached members for {room_id}. Full member sync is deferred in large rooms."
                                        )),
                                )
                            })
                            .child(if rows.is_empty() {
                                div()
                                    .size_full()
                                    .px_4()
                                    .py_4()
                                    .text_sm()
                                    .text_color(theme.text_muted)
                                    .child("No matching members")
                                    .into_any_element()
                            } else {
                                uniform_list("members-list", rows.len(), move |range, _, cx| {
                                    let theme = cx.onedark_theme();
                                    let card_radius = element_radius_small(cx);
                                    range
                                        .map(|ix| match &rows_for_list[ix] {
                                            MemberListRow::Header(label) => div()
                                                .h(MEMBER_LIST_ROW_HEIGHT)
                                                .px_4()
                                                .flex()
                                                .items_end()
                                                .pb_1()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme.text_muted)
                                                .child(label.clone())
                                                .into_any_element(),
                                            MemberListRow::Member(member) => div()
                                                .h(MEMBER_LIST_ROW_HEIGHT)
                                                .px_4()
                                                .py_2()
                                                .child(
                                                    div()
                                                        .size_full()
                                                        .px_2()
                                                        .corner_radii(Corners::all(card_radius))
                                                        .hover(|style| {
                                                            style.bg(theme.sidebar_item_hover)
                                                        })
                                                        .child(
                                                            div()
                                                                .size_full()
                                                                .flex()
                                                                .items_center()
                                                                .gap_3()
                                                                .child(Self::render_avatar_tile(
                                                                    member.avatar_url.as_ref(),
                                                                    px(36.0),
                                                                    &member.display_name,
                                                                    &member.user_id,
                                                                    ImagePriority::Low,
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
                                                                                .text_color(
                                                                                    theme.text,
                                                                                )
                                                                                .truncate()
                                                                                .child(
                                                                                    member.display_name
                                                                                        .clone(),
                                                                                ),
                                                                        )
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .text_color(
                                                                                    theme.text_muted,
                                                                                )
                                                                                .truncate()
                                                                                .child(
                                                                                    member.user_id
                                                                                        .clone(),
                                                                                ),
                                                                        ),
                                                                ),
                                                        ),
                                                )
                                                .into_any_element(),
                                        })
                                        .collect()
                                })
                                .size_full()
                                .track_scroll(scroll_handle)
                                .into_any_element()
                            })
                            .vertical_scrollbar(&self.members_scroll_handle)
                            .into_any_element()
                    }
                },
            );

        self.render_panel_frame(
            "Members",
            Some(info.room_name.clone()),
            true,
            body.into_any_element(),
            cx,
        )
    }

    fn render_pinned_messages_panel(
        &self,
        info: &RoomInfoData,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.onedark_theme();
        let card_radius = element_radius_small(cx);
        let timeline_view = self.timeline_view.clone();
        let title = match &self.pinned_messages {
            AsyncSection::Ready(entries) if entries.len() == 1 => "1 Pinned message".to_string(),
            AsyncSection::Ready(entries) => format!("{} Pinned messages", entries.len()),
            _ if info.pinned_count == 1 => "1 Pinned message".to_string(),
            _ => format!("{} Pinned messages", info.pinned_count),
        };

        let body = match &self.pinned_messages {
            AsyncSection::Idle | AsyncSection::Loading => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child("Loading pinned messages..."),
                )
                .into_any_element(),
            AsyncSection::Failed(error) => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .px_4()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .text_center()
                        .child(error.clone()),
                )
                .into_any_element(),
            AsyncSection::Ready(entries) => div()
                .size_full()
                .min_h_0()
                .overflow_y_scrollbar()
                .child(div().px_4().py_4().flex().flex_col().gap_4().children(
                    if entries.is_empty() {
                        vec![
                            div()
                                .py_3()
                                .text_sm()
                                .text_color(theme.text_muted)
                                .child("No pinned messages")
                                .into_any_element(),
                        ]
                    } else {
                        entries
                            .iter()
                            .map(|entry| {
                                let event_id = entry.event_id.clone();
                                let timeline_view = timeline_view.clone();
                                div()
                                    .pb_4()
                                    .border_b(px(1.0))
                                    .border_color(theme.border.opacity(0.5))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(theme.sidebar_item_hover.opacity(0.35)))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        let _ = timeline_view.update(cx, |this, cx| {
                                            this.jump_to_event(event_id.clone(), cx);
                                        });
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .gap_3()
                                            .child(Self::render_avatar_tile(
                                                entry.avatar_url.as_ref(),
                                                px(36.0),
                                                &entry.sender_name,
                                                &entry.sender_id,
                                                ImagePriority::Normal,
                                                cx,
                                            ))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .gap_2()
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(
                                                                        FontWeight::SEMIBOLD,
                                                                    )
                                                                    .text_color(theme.accent)
                                                                    .child(
                                                                        entry.sender_name.clone(),
                                                                    ),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(theme.text_muted)
                                                                    .child(entry.timestamp.clone()),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .px_3()
                                                            .py_3()
                                                            .corner_radii(Corners::all(card_radius))
                                                            .bg(theme.sidebar_item_hover)
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .text_color(theme.text)
                                                                    .whitespace_normal()
                                                                    .child(entry.body.clone()),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.text_muted)
                                                            .truncate()
                                                            .child(entry.event_id.clone()),
                                                    ),
                                            ),
                                    )
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    },
                ))
                .into_any_element(),
        };

        self.render_panel_frame(title, Some(info.room_name.clone()), true, body, cx)
    }
}

impl Render for RoomDetailsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();

        match self.room_info.clone() {
            AsyncSection::Ready(info) => match self.active_view {
                PanelView::RoomInfo => self.render_room_info(&info, cx),
                PanelView::Members => self.render_members_panel(&info, cx),
                PanelView::PinnedMessages => self.render_pinned_messages_panel(&info, cx),
            },
            AsyncSection::Idle | AsyncSection::Loading => div()
                .size_full()
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
                .into_any_element(),
            AsyncSection::Failed(error) => div()
                .size_full()
                .bg(theme.sidebar_background)
                .border_l(px(1.0))
                .border_color(theme.border)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .px_4()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .text_center()
                        .child(error.clone()),
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

fn build_member_groups(
    members: Vec<RoomMember>,
) -> (Vec<MemberGroup>, HashMap<String, MemberEntry>) {
    let mut entries = members
        .into_iter()
        .map(|member| {
            let user_id = member.user_id().to_string();
            let display_name = member_display_name(&member);
            let entry = MemberEntry {
                user_id: user_id.clone(),
                user_id_lower: user_id.to_lowercase(),
                display_name: display_name.clone(),
                display_name_lower: display_name.to_lowercase(),
                avatar_url: member.avatar_url().map(|url| url.to_string()),
                role: bucket_for_role(member.suggested_role_for_power_level()),
            };
            (
                entry.user_id.clone(),
                entry.display_name_lower.clone(),
                entry,
            )
        })
        .collect::<Vec<_>>();

    entries
        .sort_by(|(_, a_key, a), (_, b_key, b)| a.role.cmp(&b.role).then_with(|| a_key.cmp(b_key)));

    let mut admins = Vec::new();
    let mut moderators = Vec::new();
    let mut members_group = Vec::new();
    let mut lookup = HashMap::new();

    for (_, _, entry) in entries {
        lookup.insert(entry.user_id.clone(), entry.clone());
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

    (groups, lookup)
}

fn build_member_list_rows(groups: &[MemberGroup], query: &str) -> Vec<MemberListRow> {
    let query = query.trim().to_lowercase();
    let mut rows = Vec::new();

    for group in groups {
        let header_ix = rows.len();
        let mut member_count = 0usize;

        for member in &group.members {
            if !query.is_empty()
                && !member.display_name_lower.contains(&query)
                && !member.user_id_lower.contains(&query)
            {
                continue;
            }

            if member_count == 0 {
                rows.push(MemberListRow::Header(String::new()));
            }

            member_count += 1;
            rows.push(MemberListRow::Member(member.clone()));
        }

        if member_count == 0 {
            continue;
        }

        rows[header_ix] = MemberListRow::Header(format!("{} — {}", group.label, member_count));
    }

    rows
}

fn fallback_sender_name(sender_id: &str) -> String {
    sender_id
        .trim_start_matches('@')
        .split(':')
        .next()
        .unwrap_or(sender_id)
        .to_string()
}

fn truncate_preview(body: String) -> String {
    const LIMIT: usize = 320;
    if body.chars().count() > LIMIT {
        format!("{}…", body.chars().take(LIMIT).collect::<String>())
    } else {
        body
    }
}

fn format_preview_timestamp(timestamp: std::time::SystemTime) -> String {
    let dt: DateTime<Local> = timestamp.into();
    let now = Local::now();

    if dt.date_naive() == now.date_naive() {
        dt.format("%-I:%M %p").to_string()
    } else {
        dt.format("%-m/%-d/%y, %-I:%M %p").to_string()
    }
}

fn pinned_entry_from_timeline_event(
    event_id: String,
    event: TimelineEvent,
    member_lookup: &HashMap<String, MemberEntry>,
) -> Option<PinnedMessageEntry> {
    if event.kind.is_utd() {
        return Some(PinnedMessageEntry {
            event_id,
            sender_id: String::new(),
            sender_name: "Encrypted event".to_string(),
            avatar_url: None,
            body: "Unable to decrypt pinned message".to_string(),
            timestamp: String::new(),
        });
    }

    match event.raw().deserialize().ok()? {
        AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(message)) => {
            let sender_id = message.sender().to_string();
            let member = member_lookup.get(&sender_id);
            let sender_name = member
                .map(|member| member.display_name.clone())
                .unwrap_or_else(|| fallback_sender_name(&sender_id));
            let avatar_url = member.and_then(|member| member.avatar_url.clone());

            if let Some(original) = message.as_original() {
                let body = match &original.content.msgtype {
                    MessageType::Text(text) => text.body.clone(),
                    MessageType::Notice(notice) => notice.body.clone(),
                    MessageType::Emote(emote) => format!("* {}", emote.body),
                    MessageType::Image(image) => format!("[Image] {}", image.body),
                    MessageType::Video(video) => format!("[Video] {}", video.body),
                    MessageType::Audio(audio) => format!("[Audio] {}", audio.body),
                    MessageType::File(file) => format!("[File] {}", file.body),
                    MessageType::Location(location) => format!("[Location] {}", location.body),
                    MessageType::VerificationRequest(request) => {
                        format!("[Verification request] {}", request.body)
                    }
                    other => format!("[{}] {}", other.msgtype(), original.content.body()),
                };

                Some(PinnedMessageEntry {
                    event_id,
                    sender_id,
                    sender_name,
                    avatar_url,
                    body: truncate_preview(body),
                    timestamp: format_preview_timestamp(
                        original
                            .origin_server_ts
                            .to_system_time()
                            .unwrap_or(std::time::SystemTime::now()),
                    ),
                })
            } else {
                Some(PinnedMessageEntry {
                    event_id,
                    sender_id,
                    sender_name,
                    avatar_url,
                    body: "Pinned message unavailable".to_string(),
                    timestamp: String::new(),
                })
            }
        }
        AnySyncTimelineEvent::MessageLike(_) => Some(PinnedMessageEntry {
            event_id,
            sender_id: String::new(),
            sender_name: "Pinned event".to_string(),
            avatar_url: None,
            body: "Pinned event".to_string(),
            timestamp: String::new(),
        }),
        AnySyncTimelineEvent::State(_) => Some(PinnedMessageEntry {
            event_id,
            sender_id: String::new(),
            sender_name: "State event".to_string(),
            avatar_url: None,
            body: "Pinned state event".to_string(),
            timestamp: String::new(),
        }),
    }
}

async fn load_pinned_messages(
    room: &MatrixRoom,
    member_lookup: &HashMap<String, MemberEntry>,
) -> Vec<PinnedMessageEntry> {
    let pinned_ids = if let Some(ids) = room.pinned_event_ids() {
        ids.to_vec()
    } else {
        room.load_pinned_events()
            .await
            .ok()
            .flatten()
            .unwrap_or_default()
    };

    let mut messages = Vec::new();
    for event_id in pinned_ids {
        match room.load_or_fetch_event(&event_id, None).await {
            Ok(event) => {
                if let Some(entry) =
                    pinned_entry_from_timeline_event(event_id.to_string(), event, member_lookup)
                {
                    messages.push(entry);
                }
            }
            Err(error) => {
                tracing::warn!(?error, %event_id, "failed to load pinned event");
            }
        }
    }

    messages
}

async fn load_room_info(room: MatrixRoom) -> RoomInfoData {
    let (is_direct, display_name, latest_encryption_state, topic_event, history_visibility_event) = tokio::join!(
        room.is_direct(),
        room.display_name(),
        room.latest_encryption_state(),
        room.get_state_event_static::<RoomTopicEventContent>(),
        room.get_state_event_static::<RoomHistoryVisibilityEventContent>(),
    );

    let is_direct = is_direct.unwrap_or(false);
    let room_name = room_display_name_or_id(&room, display_name);
    let encrypted = latest_encryption_state
        .map(|state| state.is_encrypted())
        .unwrap_or_else(|_| room.encryption_state().is_encrypted());
    let topic = extract_topic(topic_event.ok().flatten());
    let history_visibility = extract_history_visibility(history_visibility_event.ok().flatten());
    let direct_profile = if is_direct {
        resolve_direct_room_profile_cached(&room).await.or_else(|| {
            room.direct_targets().iter().next().map(|target| {
                let user_id = target.to_string();
                DirectRoomProfile {
                    display_name: fallback_sender_name(&user_id),
                    avatar_url: None,
                    user_id,
                }
            })
        })
    } else {
        None
    };

    let member_count = room.active_members_count() as usize;
    let pinned_count = room
        .pinned_event_ids()
        .map(|ids| ids.len())
        .unwrap_or_default();

    let room_avatar_url = room.avatar_url().map(|url| url.to_string()).or_else(|| {
        direct_profile
            .as_ref()
            .and_then(|profile| profile.avatar_url.clone())
    });
    let hero_label = direct_profile
        .as_ref()
        .map(|profile| profile.display_name.clone())
        .unwrap_or_else(|| room_name.clone());
    let subtitle = direct_profile
        .as_ref()
        .map(|profile| profile.user_id.clone());

    RoomInfoData {
        room_name,
        room_avatar_url,
        hero_label,
        subtitle,
        topic,
        history_visibility,
        pinned_count,
        member_count,
        encrypted,
        room_id: room.room_id().to_string(),
    }
}

async fn load_members_data(room: MatrixRoom) -> Result<MembersData, String> {
    let memberships = RoomMemberships::ACTIVE;
    let local_members = room
        .members_no_sync(memberships)
        .await
        .map_err(|error| format!("Failed to load cached members: {error}"))?;

    if room.are_members_synced() {
        let (groups, member_lookup) = build_member_groups(local_members);
        return Ok(MembersData {
            groups,
            member_lookup,
            synced: true,
        });
    }

    let mut members = local_members;
    let mut synced = false;
    if room.active_members_count() <= MEMBER_LIST_SYNC_THRESHOLD
        && room.sync_members().await.is_ok()
        && let Ok(remote_members) = room.members(memberships).await
    {
        members = remote_members;
        synced = true;
    }

    let (groups, member_lookup) = build_member_groups(members);
    Ok(MembersData {
        groups,
        member_lookup,
        synced,
    })
}

async fn load_pinned_messages_data(
    room: MatrixRoom,
    member_lookup: Option<HashMap<String, MemberEntry>>,
) -> Result<Vec<PinnedMessageEntry>, String> {
    let member_lookup = match member_lookup {
        Some(member_lookup) => member_lookup,
        None => match load_members_data(room.clone()).await {
            Ok(members) => members.member_lookup,
            Err(_) => HashMap::new(),
        },
    };

    Ok(load_pinned_messages(&room, &member_lookup).await)
}
