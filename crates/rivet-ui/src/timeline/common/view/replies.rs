use crate::theme::onedark::OneDarkTheme;
use crate::timeline::RenderedReplyPreview;
use gpui::InteractiveElement as _;
use gpui::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::TimelineView;

#[derive(Clone)]
pub(crate) struct ReplyPreviewInteraction {
    pub(crate) target_event_id: String,
    pub(crate) event_indices: Arc<HashMap<String, usize>>,
    pub(crate) list_state: ListState,
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
        let event_indices = interaction.event_indices.clone();
        let list_state = interaction.list_state.clone();
        let view = interaction.view.clone();

        preview
            .cursor_pointer()
            .hover(|this| this.bg(theme.sidebar_background.opacity(0.7)))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                if let Some(index) = event_indices.get(&target_event_id).copied() {
                    list_state.scroll_to_reveal_item(index);
                    let highlight_id = target_event_id.clone();
                    let _ = view.update(cx, |this, cx| {
                        this.highlight_event(highlight_id.clone(), cx);
                    });

                    let view = view.clone();
                    let clear_id = target_event_id.clone();
                    cx.spawn(async move |cx| {
                        cx.background_executor().timer(Duration::from_secs(2)).await;
                        let _ = view.update(cx, |this, cx| {
                            if this.highlighted_event_id.as_deref() == Some(clear_id.as_str()) {
                                this.highlighted_event_id = None;
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                }
            })
            .into_any_element()
    } else {
        preview.into_any_element()
    }
}
