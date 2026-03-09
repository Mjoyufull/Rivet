use crate::theme::onedark::OneDarkTheme;
use crate::timeline::common::view::{
    ReplyPreviewInteraction, render_avatar, render_image_stack, render_message_header,
    render_message_stack,
};
use crate::timeline::{RenderedBody, RenderedReplyPreview};
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
    is_grouped: bool,
    avatar_url: Option<&String>,
    edited: bool,
    is_notice: bool,
    theme: &OneDarkTheme,
    cx: &App,
) -> AnyElement {
    div()
        .hover(|this| this.bg(theme.sidebar_item_hover.opacity(0.5)))
        .px_4()
        .py_1()
        .child(
            div()
                .flex()
                .items_start()
                .gap_4()
                .child(if !is_grouped {
                    render_avatar(avatar_url, sender_name, sender_id, px(40.0), theme, cx)
                } else {
                    div().size_10().flex_shrink_0().into_any_element()
                })
                .child(
                    div()
                        .flex_1()
                        .flex_col()
                        .gap_1()
                        .child(if !is_grouped {
                            render_message_header(
                                row_group_id,
                                sender_name,
                                sender_id,
                                timestamp,
                                theme,
                            )
                        } else {
                            div().into_any_element()
                        })
                        .child(render_message_stack(
                            item_id,
                            body,
                            reply_to,
                            reply_interaction,
                            edited,
                            is_notice,
                            theme,
                        )),
                ),
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
    is_grouped: bool,
    avatar_url: Option<&String>,
    edited: bool,
    theme: &OneDarkTheme,
    cx: &App,
) -> AnyElement {
    div()
        .hover(|this| this.bg(theme.sidebar_item_hover.opacity(0.5)))
        .px_4()
        .py_1()
        .child(
            div()
                .flex()
                .items_start()
                .gap_4()
                .child(if !is_grouped {
                    render_avatar(avatar_url, sender_name, sender_id, px(40.0), theme, cx)
                } else {
                    div().size_10().flex_shrink_0().into_any_element()
                })
                .child(
                    div()
                        .flex_1()
                        .flex_col()
                        .gap_1()
                        .child(if !is_grouped {
                            render_message_header(
                                row_group_id,
                                sender_name,
                                sender_id,
                                timestamp,
                                theme,
                            )
                        } else {
                            div().into_any_element()
                        })
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
                ),
        )
        .into_any_element()
}
