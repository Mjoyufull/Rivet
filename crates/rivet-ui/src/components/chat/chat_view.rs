use super::{MessageInput, RoomDetailsPanel};
use crate::components::remote_image::avatar_fallback_label;
use crate::models::appearance::avatar_radius_for;
use crate::rooms::RoomListModel;
use crate::theme::onedark::OneDarkThemeExt;
use crate::timeline::{TimelineModel, TimelineView};
use gpui::*;
use gpui_component::StyledExt;

pub struct ChatView {
    room_list_model: Entity<RoomListModel>,
    timeline_model: Entity<TimelineModel>,
    timeline_view: Entity<TimelineView>,
    room_details_panel: Entity<RoomDetailsPanel>,
    message_input: Entity<MessageInput>,
}

impl ChatView {
    pub fn new(
        room_list_model: Entity<RoomListModel>,
        timeline_model: Entity<TimelineModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let timeline_view = cx.new(|cx| TimelineView::new(timeline_model.clone(), cx));
        let room = timeline_model.read(cx).room.clone();
        let room_details_panel = cx.new(|cx| RoomDetailsPanel::new(room, window, cx));
        let message_input = cx.new(|cx| MessageInput::new(timeline_model.clone(), window, cx));

        Self {
            room_list_model,
            timeline_model,
            timeline_view,
            room_details_panel,
            message_input,
        }
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let room_id = self.timeline_model.read(cx).room.room_id().to_string();

        let room_info = self
            .room_list_model
            .read(cx)
            .all_rooms
            .iter()
            .find(|r| r.id == room_id)
            .cloned();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(
                // Header
                div()
                    .h_12()
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b(px(1.0))
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
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
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme.text)
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
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(self.timeline_view.clone())
                            .child(self.message_input.clone()),
                    )
                    .child(self.room_details_panel.clone()),
            )
    }
}
