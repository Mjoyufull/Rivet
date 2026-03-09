use crate::theme::onedark::OneDarkTheme;
use crate::timeline::common::view::{
    ReplyPreviewInteraction, render_image_stack, render_message_header, render_message_stack,
};
use crate::timeline::{RenderedBody, RenderedReplyPreview};
use gpui::prelude::FluentBuilder;
use gpui::*;

pub(crate) fn render_message(
    item_id: &str,
    row_group_id: &SharedString,
    sender_id: &str,
    sender_name: &str,
    body: &RenderedBody,
    reply_to: Option<&RenderedReplyPreview>,
    reply_interaction: Option<ReplyPreviewInteraction>,
    timestamp: &str,
    is_own: bool,
    is_grouped: bool,
    edited: bool,
    is_notice: bool,
    theme: &OneDarkTheme,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .map(|this| if is_own { this.items_end() } else { this })
        .max_w(rems(46.0))
        .py_2()
        .gap_1()
        .when(!is_grouped, |this| {
            this.child(render_message_header(
                row_group_id,
                sender_name,
                sender_id,
                timestamp,
                theme,
            ))
        })
        .child(
            div()
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(if is_own {
                    theme.accent.opacity(0.2)
                } else {
                    theme.sidebar_background
                })
                .border_1()
                .border_color(theme.border)
                .child(render_message_stack(
                    item_id,
                    body,
                    reply_to,
                    reply_interaction,
                    edited,
                    is_notice,
                    theme,
                )),
        )
        .into_any_element()
}

pub(crate) fn render_image(
    item_id: &str,
    row_group_id: &SharedString,
    sender_id: &str,
    sender_name: &str,
    source: &matrix_sdk::ruma::events::room::MediaSource,
    mimetype: Option<&String>,
    dimensions: Option<(u32, u32)>,
    caption: Option<&String>,
    reply_to: Option<&RenderedReplyPreview>,
    reply_interaction: Option<ReplyPreviewInteraction>,
    timestamp: &str,
    is_own: bool,
    is_grouped: bool,
    edited: bool,
    theme: &OneDarkTheme,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .map(|this| if is_own { this.items_end() } else { this })
        .max_w(rems(46.0))
        .py_2()
        .gap_1()
        .when(!is_grouped, |this| {
            this.child(render_message_header(
                row_group_id,
                sender_name,
                sender_id,
                timestamp,
                theme,
            ))
        })
        .child(
            div()
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(if is_own {
                    theme.accent.opacity(0.2)
                } else {
                    theme.sidebar_background
                })
                .border_1()
                .border_color(theme.border)
                .child(render_image_stack(
                    item_id,
                    source,
                    mimetype,
                    dimensions,
                    caption.map(|c| RenderedBody::Plain(c.to_string())).as_ref(),
                    reply_to,
                    reply_interaction,
                    edited,
                    theme,
                )),
        )
        .into_any_element()
}
