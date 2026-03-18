use super::{DetailsPanelPreferences, DetailsTopBarMode, MessageInput, RoomDetailsPanel};
use crate::components::remote_image::avatar_fallback_label;
use crate::models::appearance::avatar_radius_for;
use crate::rooms::RoomListModel;
use crate::theme::onedark::{OneDarkTheme, OneDarkThemeExt};
use crate::timeline::{TimelineModel, TimelineView};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};

const DETAILS_PANEL_DEFAULT_WIDTH: f32 = 320.0;
const DETAILS_PANEL_MIN_WIDTH: f32 = 280.0;
const DETAILS_PANEL_MAX_WIDTH: f32 = 420.0;
const DETAILS_RESIZE_HANDLE_WIDTH: f32 = 8.0;

#[derive(Debug, Clone)]
struct DraggedDetailsPanelHandle;

pub struct ChatView {
    room_list_model: Entity<RoomListModel>,
    timeline_model: Entity<TimelineModel>,
    timeline_view: Entity<TimelineView>,
    room_details_panel: Entity<RoomDetailsPanel>,
    message_input: Entity<MessageInput>,
    details_panel_width: Pixels,
}

impl ChatView {
    pub fn new(
        room_list_model: Entity<RoomListModel>,
        timeline_model: Entity<TimelineModel>,
        details_panel_preferences: DetailsPanelPreferences,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let timeline_view = cx.new(|cx| TimelineView::new(timeline_model.clone(), cx));
        let room = timeline_model.read(cx).room.clone();
        let room_id = room.room_id().to_string();
        let is_direct = room_list_model
            .read(cx)
            .all_rooms
            .iter()
            .find(|info| info.id == room_id)
            .map(|info| info.is_direct)
            .unwrap_or(false);
        let room_details_panel = cx.new(|cx| {
            RoomDetailsPanel::new(
                room,
                is_direct,
                details_panel_preferences.clone(),
                timeline_view.clone(),
                window,
                cx,
            )
        });
        let message_input = cx.new(|cx| MessageInput::new(timeline_model.clone(), window, cx));

        Self {
            room_list_model,
            timeline_model,
            timeline_view,
            room_details_panel,
            message_input,
            details_panel_width: px(DETAILS_PANEL_DEFAULT_WIDTH),
        }
    }

    fn render_details_resize_handle(&self, theme: OneDarkTheme) -> AnyElement {
        div()
            .relative()
            .h_full()
            .flex_shrink_0()
            .w(px(1.0))
            .bg(theme.border)
            .child(
                div()
                    .id("details-resize-handle")
                    .absolute()
                    .left(px(-(DETAILS_RESIZE_HANDLE_WIDTH / 2.0)))
                    .w(px(DETAILS_RESIZE_HANDLE_WIDTH))
                    .h_full()
                    .cursor_col_resize()
                    .block_mouse_except_scroll()
                    .on_drag(DraggedDetailsPanelHandle, |_, _, _, cx| cx.new(|_| Empty)),
            )
            .into_any_element()
    }

    fn resize_details_panel(
        &mut self,
        drag_event: &DragMoveEvent<DraggedDetailsPanelHandle>,
        cx: &mut Context<Self>,
    ) {
        let bounds = drag_event.bounds;
        let drag_position = drag_event.event.position;
        let next_width = bounds.right() - drag_position.x;
        self.details_panel_width =
            next_width.clamp(px(DETAILS_PANEL_MIN_WIDTH), px(DETAILS_PANEL_MAX_WIDTH));
        cx.notify();
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.onedark_theme();
        let room_id = self.timeline_model.read(cx).room.room_id().to_string();
        let selected_button = ButtonCustomVariant::new(cx)
            .color(theme.sidebar_item_active)
            .foreground(theme.text)
            .border(theme.accent.opacity(0.35))
            .hover(theme.sidebar_item_hover)
            .active(theme.sidebar_item_active.opacity(0.9));
        let idle_button = ButtonCustomVariant::new(cx)
            .color(theme.sidebar_background)
            .foreground(theme.text_muted)
            .border(theme.border)
            .hover(theme.sidebar_item_hover)
            .active(theme.sidebar_item_active);

        let room_info = self
            .room_list_model
            .read(cx)
            .all_rooms
            .iter()
            .find(|r| r.id == room_id)
            .cloned();

        let top_bar_mode = self.room_details_panel.read(cx).top_bar_mode();
        let details_visible = self.room_details_panel.read(cx).is_visible();
        let room_info_panel = self.room_details_panel.clone();
        let members_panel = self.room_details_panel.clone();
        let room_info_button = Button::new("toggle-room-info")
            .label("Room Info")
            .icon(gpui_component::IconName::Info)
            .custom(
                if details_visible && matches!(top_bar_mode, DetailsTopBarMode::RoomInfo) {
                    selected_button
                } else {
                    idle_button
                },
            )
            .on_click(move |_, _, cx| {
                let _ = room_info_panel.update(cx, |this, cx| {
                    this.toggle_room_info(cx);
                });
            });

        let members_button = Button::new("toggle-members")
            .label("Members")
            .icon(gpui_component::IconName::User)
            .custom(
                if details_visible && matches!(top_bar_mode, DetailsTopBarMode::Members) {
                    selected_button
                } else {
                    idle_button
                },
            )
            .on_click(move |_, _, cx| {
                let _ = members_panel.update(cx, |this, cx| {
                    this.toggle_members(cx);
                });
            });

        let center_column = div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(div().size_full().child(self.timeline_view.clone())),
            )
            .child(div().flex_shrink_0().child(self.message_input.clone()));
        let details_panel_width = self
            .details_panel_width
            .clamp(px(DETAILS_PANEL_MIN_WIDTH), px(DETAILS_PANEL_MAX_WIDTH));

        div()
            .size_full()
            .min_w_0()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(
                div()
                    .h_12()
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .border_b(px(1.0))
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .min_w_0()
                            .child(
                                if let Some(url) =
                                    room_info.as_ref().and_then(|r| r.avatar_url.clone())
                                {
                                    let fallback = room_info
                                        .as_ref()
                                        .map(|r| avatar_fallback_label(&r.name, &r.id))
                                        .unwrap_or_else(|| "?".to_string());
                                    crate::components::remote_image::RemoteImage::new(url)
                                        .size(px(40.0))
                                        .avatar()
                                        .high_priority()
                                        .fallback_text(fallback)
                                        .into_any_element()
                                } else {
                                    div()
                                        .size_10()
                                        .corner_radii(Corners::all(avatar_radius_for(px(40.0), cx)))
                                        .bg(theme.accent.opacity(0.2))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            div().text_sm().text_color(theme.accent).child(
                                                room_info
                                                    .as_ref()
                                                    .map(|r| {
                                                        r.name
                                                            .trim_start_matches('@')
                                                            .chars()
                                                            .next()
                                                            .unwrap_or('?')
                                                            .to_string()
                                                    })
                                                    .unwrap_or_else(|| "?".to_string()),
                                            ),
                                        )
                                        .into_any_element()
                                },
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme.text)
                                            .truncate()
                                            .child(
                                                room_info
                                                    .as_ref()
                                                    .map(|r| r.name.clone())
                                                    .unwrap_or_else(|| "Loading...".to_string()),
                                            ),
                                    )
                                    .child(div().text_xs().text_color(theme.text_muted).child(
                                        if room_info.as_ref().map(|r| r.is_direct).unwrap_or(false)
                                        {
                                            "Direct Message"
                                        } else {
                                            "Room"
                                        },
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(room_info_button)
                            .child(members_button),
                    ),
            )
            .child(
                div().flex_1().min_w_0().min_h_0().overflow_hidden().child(
                    div()
                        .size_full()
                        .flex()
                        .min_w_0()
                        .min_h_0()
                        .on_drag_move::<DraggedDetailsPanelHandle>(cx.listener(
                            |this, event, _window, cx| {
                                this.resize_details_panel(event, cx);
                            },
                        ))
                        .child(div().flex_1().min_w_0().min_h_0().child(center_column))
                        .when(details_visible, |this| {
                            this.child(self.render_details_resize_handle(theme)).child(
                                div()
                                    .w(details_panel_width)
                                    .h_full()
                                    .flex_shrink_0()
                                    .child(self.room_details_panel.clone()),
                            )
                        }),
                ),
            )
    }
}
