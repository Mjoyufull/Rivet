use crate::theme::onedark::OneDarkTheme;
use crate::timeline::RenderedReplyPreview;
use gpui::InteractiveElement as _;
use gpui::*;

use super::TimelineView;

#[derive(Clone)]
pub(crate) struct ReplyPreviewInteraction {
    pub(crate) target_event_id: String,
    pub(crate) view: Entity<TimelineView>,
}

pub(crate) fn render_reply_preview(
    reply: &RenderedReplyPreview,
    interaction: Option<ReplyPreviewInteraction>,
    theme: &OneDarkTheme,
) -> AnyElement {
    let preview = div()
        .rounded_md()
        .border_l_2()
        .border_color(theme.accent.opacity(0.8))
        .bg(theme.sidebar_background.opacity(0.45))
        .px_3()
        .py_2()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.accent)
                .child(reply.sender_name.clone()),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .line_clamp(1)
                .child(reply.body.clone()),
        );

    if let Some(interaction) = interaction {
        let target_event_id = interaction.target_event_id.clone();
        let view = interaction.view.clone();

        preview
            .cursor_pointer()
            .hover(|this| this.bg(theme.sidebar_background.opacity(0.7)))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                let _ = view.update(cx, |this, cx| {
                    this.jump_to_event(target_event_id.clone(), cx);
                });
            })
            .into_any_element()
    } else {
        preview.into_any_element()
    }
}
