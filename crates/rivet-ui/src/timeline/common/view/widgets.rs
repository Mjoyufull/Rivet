use crate::models::appearance::avatar_radius_for;
use crate::theme::onedark::OneDarkTheme;
use crate::timeline::{RenderedBody, RenderedCallEntry, RenderedReplyPreview};
use gpui::InteractiveElement as _;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::Sizable;
use gpui_component::StyledExt;

use super::body::render_body;
use super::replies::{ReplyPreviewInteraction, render_reply_preview};
use super::{TimelineView, render_avatar};

fn render_edited_indicator(theme: &OneDarkTheme) -> AnyElement {
    div()
        .mt_1()
        .text_xs()
        .italic()
        .text_color(theme.text_muted)
        .child("(edited)")
        .into_any_element()
}

pub(crate) fn render_message_header(
    row_group_id: &SharedString,
    sender_name: &str,
    sender_id: &str,
    timestamp: &str,
    theme: &OneDarkTheme,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .min_w_0()
                .child(
                    div()
                        .group(row_group_id.clone())
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.accent)
                        .cursor_pointer()
                        .hover(|this| this.text_decoration_1().text_decoration_color(theme.accent))
                        .child(sender_name.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(timestamp.to_string()),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .whitespace_nowrap()
                .truncate()
                .max_w(rems(18.0))
                .invisible()
                .group_hover(row_group_id.clone(), |this| this.visible())
                .child(sender_id.to_string()),
        )
        .into_any_element()
}

pub(crate) fn render_message_stack(
    item_id: &str,
    body: &RenderedBody,
    reply_to: Option<&RenderedReplyPreview>,
    reply_interaction: Option<ReplyPreviewInteraction>,
    edited: bool,
    is_notice: bool,
    theme: &OneDarkTheme,
) -> AnyElement {
    div()
        .flex_col()
        .gap_1()
        .when_some(reply_to, |this, reply| {
            this.child(render_reply_preview(
                reply,
                reply_interaction.clone(),
                theme,
            ))
        })
        .child(render_body(item_id, body, is_notice, theme))
        .when(edited, |this| this.child(render_edited_indicator(theme)))
        .into_any_element()
}

pub(crate) fn render_image_stack(
    item_id: &str,
    source: &matrix_sdk::ruma::events::room::MediaSource,
    mimetype: Option<&String>,
    dimensions: Option<(u32, u32)>,
    caption: Option<&RenderedBody>,
    reply_to: Option<&RenderedReplyPreview>,
    reply_interaction: Option<ReplyPreviewInteraction>,
    edited: bool,
    theme: &OneDarkTheme,
) -> AnyElement {
    let (frame_width, frame_height) = image_frame_size(dimensions);
    let image = crate::components::remote_image::RemoteImage::new(
        crate::timeline::common::model::room_media_source_url(source),
    )
    .with_source(source.clone())
    .with_mimetype(mimetype.cloned())
    .frame_size(frame_width, frame_height)
    .high_priority()
    .object_fit(ObjectFit::Contain);
    div()
        .flex_col()
        .gap_2()
        .when_some(reply_to, |this, reply| {
            this.child(render_reply_preview(
                reply,
                reply_interaction.clone(),
                theme,
            ))
        })
        .child(
            div()
                .rounded_lg()
                .overflow_hidden()
                .border_1()
                .border_color(theme.border)
                .w(frame_width)
                .h(frame_height)
                .max_w(frame_width)
                .max_h(frame_height)
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.sidebar_background.opacity(0.55))
                .child(image.into_any_element()),
        )
        .when_some(caption, |this, caption| {
            this.child(render_body(item_id, caption, false, theme))
        })
        .when(edited, |this| this.child(render_edited_indicator(theme)))
        .into_any_element()
}

fn image_frame_size(dimensions: Option<(u32, u32)>) -> (Pixels, Pixels) {
    const REM_PX: f32 = 16.0;
    const MAX_WIDTH: f32 = 28.0 * REM_PX;
    const MAX_HEIGHT: f32 = 20.0 * REM_PX;
    const MIN_WIDTH: f32 = 12.0 * REM_PX;
    const MIN_HEIGHT: f32 = 8.0 * REM_PX;

    if let Some((width, height)) = dimensions
        && width > 0
        && height > 0
    {
        let width = width as f32;
        let height = height as f32;
        let scale = (MAX_WIDTH / width).min(MAX_HEIGHT / height).min(1.0);

        return (
            px((width * scale).clamp(MIN_WIDTH, MAX_WIDTH)),
            px((height * scale).clamp(MIN_HEIGHT, MAX_HEIGHT)),
        );
    }

    (px(MAX_WIDTH), px(MAX_HEIGHT))
}

pub(crate) fn render_system_row(
    content: &str,
    timestamp: &str,
    icon_path: Option<&str>,
    theme: &OneDarkTheme,
) -> AnyElement {
    let icon_slot = div()
        .w(px(16.0))
        .flex_none()
        .flex()
        .justify_center()
        .pt(px(1.0))
        .child(
            icon_path
                .map(|path| {
                    gpui_component::Icon::empty()
                        .path(path.to_string())
                        .with_size(gpui_component::Size::Small)
                        .text_color(theme.accent)
                        .into_any_element()
                })
                .unwrap_or_else(|| div().w(px(16.0)).h(px(16.0)).into_any_element()),
        );

    let mut row = div()
        .flex()
        .items_start()
        .gap_3()
        .text_xs()
        .child(icon_slot);

    row = row
        .child(
            div()
                .flex_1()
                .text_color(theme.text_muted)
                .child(render_body(
                    "system",
                    &RenderedBody::Plain(content.to_string()),
                    true,
                    theme,
                )),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .whitespace_nowrap()
                .child(timestamp.to_string()),
        );

    div().px_5().py_1().child(row).into_any_element()
}

pub(crate) fn render_centered_divider(text: &str, theme: &OneDarkTheme) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_4()
        .px_4()
        .py_4()
        .child(div().h_px().flex_1().bg(theme.border.opacity(0.8)))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_muted)
                .child(text.to_string()),
        )
        .child(div().h_px().flex_1().bg(theme.border.opacity(0.8)))
        .into_any_element()
}

pub(crate) fn render_timeline_intro(
    room_name: &str,
    avatar_url: Option<&String>,
    theme: &OneDarkTheme,
    cx: &App,
) -> AnyElement {
    div()
        .px_6()
        .pt_6()
        .pb_3()
        .child(
            div()
                .flex_col()
                .gap_4()
                .child(render_avatar(
                    avatar_url,
                    room_name,
                    room_name,
                    px(64.0),
                    theme,
                    cx,
                ))
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text)
                        .child(room_name.to_string()),
                )
                .child(div().text_sm().text_color(theme.text_muted).child(format!(
                    "This is the beginning of your message history in {room_name}."
                )))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child("Older history will appear above when it is available."),
                ),
        )
        .into_any_element()
}

pub(crate) fn render_call_group(
    id: &str,
    sender_name: &str,
    label: &str,
    entries: &[RenderedCallEntry],
    expanded: bool,
    view: Entity<TimelineView>,
    theme: &OneDarkTheme,
    cx: &App,
) -> AnyElement {
    let row_group_id = SharedString::from(format!("call-group-{id}"));
    let id = id.to_string();
    let count_label = format!("{}x", entries.len());
    let entry_rows = entries.iter().map(|entry| {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px_3()
            .py_1()
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child(entry.content.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .whitespace_nowrap()
                    .child(entry.timestamp.clone()),
            )
            .into_any_element()
    });

    div()
        .group(row_group_id.clone())
        .px_4()
        .py_2()
        .child(
            div()
                .rounded_lg()
                .bg(theme.sidebar_background.opacity(0.55))
                .border_1()
                .border_color(theme.border.opacity(0.5))
                .child(
                    div()
                        .px_3()
                        .py_3()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .size(px(32.0))
                                .flex_shrink_0()
                                .corner_radii(Corners::all(avatar_radius_for(px(32.0), cx)))
                                .bg(theme.accent.opacity(0.2))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    gpui_component::Icon::empty()
                                        .path("icons/phone-call.svg")
                                        .text_color(theme.accent),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex_col()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text)
                                        .child(sender_name.to_string()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child(label.to_string()),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child(count_label),
                        )
                        .child(
                            div()
                                .cursor_pointer()
                                .text_color(theme.text_muted)
                                .hover(|this| this.text_color(theme.text))
                                .child(if expanded {
                                    gpui_component::IconName::ChevronDown
                                } else {
                                    gpui_component::IconName::ChevronRight
                                })
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.toggle_call_group(&id, cx);
                                    });
                                }),
                        ),
                )
                .when(expanded, |this| {
                    this.child(
                        div()
                            .border_t_1()
                            .border_color(theme.border.opacity(0.5))
                            .children(entry_rows),
                    )
                }),
        )
        .into_any_element()
}
