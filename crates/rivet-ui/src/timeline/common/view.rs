mod body;
mod replies;
mod widgets;

use crate::components::remote_image::avatar_fallback_label;
use crate::models::appearance::avatar_radius_for;
use crate::theme::onedark::{OneDarkTheme, OneDarkThemeExt};
use crate::timeline::{ChatStyle, RenderedTimelineItem, TimelineModel};
use crate::timeline::{bubble, modern};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::scroll::Scrollbar;
use std::collections::HashSet;
use std::time::Duration;

pub(crate) use replies::ReplyPreviewInteraction;
use widgets::{
    render_call_group, render_centered_divider, render_system_row, render_timeline_intro,
};
pub(crate) use widgets::{render_image_stack, render_message_header, render_message_stack};

const TIMELINE_SCROLLBAR_GUTTER_WIDTH: f32 = 12.0;

pub struct TimelineView {
    model: Entity<TimelineModel>,
    highlighted_event_id: Option<String>,
    expanded_call_groups: HashSet<String>,
}

const EVENT_HIGHLIGHT_DURATION: Duration = Duration::from_millis(700);
const JUMP_PAGINATION_POLL_DELAY: Duration = Duration::from_millis(120);
const MAX_EVENT_JUMP_ATTEMPTS: usize = 24;

impl TimelineView {
    pub fn new(model: Entity<TimelineModel>, _cx: &mut Context<Self>) -> Self {
        Self {
            model,
            highlighted_event_id: None,
            expanded_call_groups: HashSet::new(),
        }
    }

    fn toggle_call_group(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.expanded_call_groups.insert(id.to_string()) {
            self.expanded_call_groups.remove(id);
        }
        cx.notify();
    }

    fn highlight_event(&mut self, event_id: String, cx: &mut Context<Self>) {
        self.highlighted_event_id = Some(event_id);
        cx.notify();
    }

    fn intro_row_count(&self, cx: &App) -> usize {
        let model = self.model.read(cx);
        if model.hit_timeline_start && !model.rendered_items.is_empty() {
            2
        } else {
            0
        }
    }

    fn loaded_event_index(&self, event_id: &str, cx: &App) -> Option<usize> {
        let intro_row_count = self.intro_row_count(cx);
        self.model
            .read(cx)
            .rendered_items
            .iter()
            .enumerate()
            .find_map(|(ix, item)| {
                (rendered_item_id(item) == Some(event_id)).then_some(ix + intro_row_count)
            })
    }

    fn clear_highlight_later(&self, event_id: String, cx: &mut Context<Self>) {
        let view = cx.entity().clone();
        cx.spawn(async move |_this: WeakEntity<Self>, cx| {
            cx.background_executor()
                .timer(EVENT_HIGHLIGHT_DURATION)
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.highlighted_event_id.as_deref() == Some(event_id.as_str()) {
                    this.highlighted_event_id = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn jump_to_loaded_event(&mut self, event_id: &str, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.loaded_event_index(event_id, cx) else {
            return false;
        };

        let list_state = self.model.read(cx).list_state.clone();
        list_state.scroll_to_reveal_item(index);
        self.highlight_event(event_id.to_string(), cx);
        self.clear_highlight_later(event_id.to_string(), cx);
        true
    }

    pub(crate) fn jump_to_event(&mut self, event_id: String, cx: &mut Context<Self>) {
        #[derive(Clone, Copy)]
        enum JumpStep {
            Done,
            Wait,
            Exhausted,
        }

        if self.jump_to_loaded_event(&event_id, cx) {
            return;
        }

        let view = cx.entity().downgrade();
        let target_event_id = event_id.clone();
        cx.spawn(async move |_this: WeakEntity<Self>, cx| {
            for _ in 0..MAX_EVENT_JUMP_ATTEMPTS {
                let Some(view) = view.upgrade() else {
                    return;
                };

                let step: Option<JumpStep> = view
                    .update(cx, |this, cx| {
                        if this.jump_to_loaded_event(&target_event_id, cx) {
                            return JumpStep::Done;
                        }

                        let (loading_history, hit_timeline_start, model) = {
                            let model = this.model.read(cx);
                            (
                                model.loading_history,
                                model.hit_timeline_start,
                                this.model.clone(),
                            )
                        };

                        if hit_timeline_start {
                            return JumpStep::Exhausted;
                        }

                        if !loading_history {
                            let handle = model.clone();
                            let _ = model.update(cx, |model, cx| {
                                model.load_more_history(handle.clone(), cx);
                            });
                        }

                        JumpStep::Wait
                    })
                    .ok();

                match step {
                    Some(JumpStep::Done) | Some(JumpStep::Exhausted) | None => return,
                    Some(JumpStep::Wait) => {
                        cx.background_executor()
                            .timer(JUMP_PAGINATION_POLL_DELAY)
                            .await;
                    }
                }
            }
        })
        .detach();
    }
}

fn rendered_item_id(item: &RenderedTimelineItem) -> Option<&str> {
    match item {
        RenderedTimelineItem::Message { id, .. }
        | RenderedTimelineItem::Image { id, .. }
        | RenderedTimelineItem::System { id, .. }
        | RenderedTimelineItem::CallGroup { id, .. } => Some(id),
        RenderedTimelineItem::Separator(_) => None,
    }
}

fn avatar_fallback(sender_name: &str, sender_id: &str) -> String {
    sender_name
        .chars()
        .find(|c| c.is_alphanumeric())
        .or_else(|| {
            sender_id
                .trim_start_matches('@')
                .chars()
                .find(|c| c.is_alphanumeric())
        })
        .unwrap_or('?')
        .to_string()
        .to_uppercase()
}

pub(crate) fn render_avatar(
    avatar_url: Option<&String>,
    sender_name: &str,
    sender_id: &str,
    size: Pixels,
    theme: &OneDarkTheme,
    cx: &App,
) -> AnyElement {
    if let Some(url) = avatar_url {
        let fallback = avatar_fallback_label(sender_name, sender_id);
        crate::components::remote_image::RemoteImage::new(url.clone())
            .size(size)
            .avatar()
            .fallback_text(fallback)
            .into_any_element()
    } else {
        div()
            .size(size)
            .flex_shrink_0()
            .corner_radii(Corners::all(avatar_radius_for(size, cx)))
            .bg(theme.accent.opacity(0.2))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.accent)
                    .child(avatar_fallback(sender_name, sender_id)),
            )
            .into_any_element()
    }
}

impl Render for TimelineView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model_handle = self.model.clone();
        let (room, rendered_items, list_state, chat_style, loading_history, hit_timeline_start) = {
            let model_read = model_handle.read(cx);
            (
                model_read.room.clone(),
                model_read.rendered_items.clone(),
                model_read.list_state.clone(),
                model_read.chat_style,
                model_read.loading_history,
                model_read.hit_timeline_start,
            )
        };
        let has_rendered_items = !rendered_items.is_empty();
        let show_start_of_conversation = hit_timeline_start && has_rendered_items;
        let intro_row_count = if show_start_of_conversation { 2 } else { 0 };
        let total_row_count = rendered_items.len() + intro_row_count;

        if list_state.item_count() != total_row_count {
            list_state.reset(total_row_count);
        }

        tracing::info!(
            "timeline_view: rendering {} items with list_count={}",
            rendered_items.len(),
            list_state.item_count()
        );
        let theme = *cx.onedark_theme();
        let highlighted_event_id = self.highlighted_event_id.clone();
        let expanded_call_groups = self.expanded_call_groups.clone();
        let room_name = room
            .cached_display_name()
            .map(|name| name.to_string())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "Conversation".to_string());
        let room_avatar_url = room.avatar_url().map(|url| url.to_string());
        let rendered_items_for_list = rendered_items.clone();
        let view = cx.entity().clone();
        let room_name_for_rows = room_name.clone();
        let room_avatar_url_for_rows = room_avatar_url.clone();

        let timeline_list = list(list_state.clone(), move |ix, _window, cx| {
            if show_start_of_conversation {
                if ix == 0 {
                    return render_timeline_intro(
                        &room_name_for_rows,
                        room_avatar_url_for_rows.as_ref(),
                        cx.onedark_theme(),
                        cx,
                    );
                }

                if ix == 1 {
                    return render_centered_divider("Start of messages", cx.onedark_theme());
                }
            }

            let item_ix = ix.saturating_sub(intro_row_count);
            let Some(item) = rendered_items_for_list.get(item_ix) else {
                return div()
                    .px_4()
                    .py_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.onedark_theme().text_muted)
                            .child("Loading..."),
                    )
                    .into_any_element();
            };
            let theme = *cx.onedark_theme();
            let row_group_id = SharedString::from(format!("timeline-row-{item_ix}"));
            let highlighted = rendered_item_id(item)
                .and_then(|id| {
                    highlighted_event_id
                        .as_deref()
                        .map(|selected| selected == id)
                })
                .unwrap_or(false);

            let row = match item {
                RenderedTimelineItem::Message {
                    id,
                    sender_id,
                    sender_name,
                    body,
                    timestamp,
                    is_own,
                    is_grouped,
                    avatar_url,
                    edited,
                    is_notice,
                    reply_to,
                } => match chat_style {
                    ChatStyle::Bubble => bubble::render_message(
                        id,
                        &row_group_id,
                        sender_id,
                        sender_name,
                        body,
                        reply_to.as_ref(),
                        reply_to.as_ref().map(|reply| ReplyPreviewInteraction {
                            target_event_id: reply.event_id.clone(),
                            view: view.clone(),
                        }),
                        timestamp,
                        *is_own,
                        *is_grouped,
                        *edited,
                        *is_notice,
                        &theme,
                    ),
                    ChatStyle::Modern => modern::render_message(
                        id,
                        &row_group_id,
                        sender_id,
                        sender_name,
                        body,
                        reply_to.as_ref(),
                        reply_to.as_ref().map(|reply| ReplyPreviewInteraction {
                            target_event_id: reply.event_id.clone(),
                            view: view.clone(),
                        }),
                        timestamp,
                        *is_grouped,
                        avatar_url.as_ref(),
                        *edited,
                        *is_notice,
                        &theme,
                        cx,
                    ),
                },
                RenderedTimelineItem::Image {
                    id,
                    sender_id,
                    sender_name,
                    source,
                    mimetype,
                    dimensions,
                    caption,
                    timestamp,
                    is_own,
                    is_grouped,
                    avatar_url,
                    edited,
                    reply_to,
                } => match chat_style {
                    ChatStyle::Bubble => bubble::render_image(
                        id,
                        &row_group_id,
                        sender_id,
                        sender_name,
                        source,
                        mimetype.as_ref(),
                        *dimensions,
                        caption.as_ref(),
                        reply_to.as_ref(),
                        reply_to.as_ref().map(|reply| ReplyPreviewInteraction {
                            target_event_id: reply.event_id.clone(),
                            view: view.clone(),
                        }),
                        timestamp,
                        *is_own,
                        *is_grouped,
                        *edited,
                        &theme,
                    ),
                    ChatStyle::Modern => modern::render_image(
                        id,
                        &row_group_id,
                        sender_id,
                        sender_name,
                        source,
                        mimetype.as_ref(),
                        *dimensions,
                        caption.as_ref(),
                        reply_to.as_ref(),
                        reply_to.as_ref().map(|reply| ReplyPreviewInteraction {
                            target_event_id: reply.event_id.clone(),
                            view: view.clone(),
                        }),
                        timestamp,
                        *is_grouped,
                        avatar_url.as_ref(),
                        *edited,
                        &theme,
                        cx,
                    ),
                },
                RenderedTimelineItem::System {
                    content,
                    timestamp,
                    arrow,
                    ..
                } => render_system_row(content, timestamp, arrow.as_deref(), &theme),
                RenderedTimelineItem::CallGroup {
                    id,
                    sender_name,
                    label,
                    entries,
                } => render_call_group(
                    id,
                    sender_name,
                    label,
                    entries,
                    expanded_call_groups.contains(id),
                    view.clone(),
                    &theme,
                    cx,
                ),
                RenderedTimelineItem::Separator(text) => render_centered_divider(text, &theme),
            };

            div()
                .bg(if highlighted {
                    theme.accent.opacity(0.14)
                } else {
                    transparent_black()
                })
                .child(row)
                .into_any_element()
        })
        .size_full()
        .map(|this| if loading_history { this.pt_10() } else { this });

        div()
            .relative()
            .size_full()
            .min_h_0()
            .w_full()
            .overflow_hidden()
            .child(
                div()
                    .size_full()
                    .flex()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .child(if !has_rendered_items {
                                div()
                                    .size_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.text_muted)
                                            .child("Syncing timeline..."),
                                    )
                                    .into_any_element()
                            } else {
                                timeline_list.into_any_element()
                            }),
                    )
                    .child(
                        div()
                            .w(px(TIMELINE_SCROLLBAR_GUTTER_WIDTH))
                            .h_full()
                            .flex_shrink_0()
                            .when(has_rendered_items, |this| {
                                this.child(
                                    Scrollbar::vertical(&list_state).id("timeline-scrollbar-right"),
                                )
                            }),
                    ),
            )
            .when(loading_history, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right(px(TIMELINE_SCROLLBAR_GUTTER_WIDTH))
                        .h_10()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(theme.background.opacity(0.94))
                        .border_b_1()
                        .border_color(theme.border.opacity(0.6))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("Loading older messages..."),
                        ),
                )
            })
    }
}
