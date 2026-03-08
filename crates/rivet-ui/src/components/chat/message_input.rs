use gpui::*;
// use gpui::prelude::*;
use crate::theme::onedark::OneDarkThemeExt;
use crate::timeline::TimelineModel;
use gpui_component::input::{Input, InputEvent, InputState};

pub struct MessageInput {
    model: Entity<TimelineModel>,
    input_state: Entity<InputState>,
}

impl MessageInput {
    pub fn new(model: Entity<TimelineModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Type a message..."));
        cx.subscribe_in(
            &input_state,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { secondary: false } = event {
                    this.handle_submit(window, cx);
                }
            },
        )
        .detach();

        Self { model, input_state }
    }

    fn handle_submit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let content = self.input_state.read(cx).text().to_string();
        if !content.is_empty() {
            let timeline = self.model.read(cx).timeline.clone();
            TimelineModel::send(timeline, content, cx);

            let _ = self
                .input_state
                .update(cx, |this, cx: &mut Context<InputState>| {
                    this.set_value("", _window, cx);
                });
        }
    }
}

impl Render for MessageInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let view = cx.entity().clone();

        div()
            .flex()
            .items_end()
            .gap_2()
            .p_4()
            .bg(theme.sidebar_background)
            .border_t_1()
            .border_color(theme.border)
            .child(div().flex_1().child(Input::new(&self.input_state)))
            .child(
                div()
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(theme.accent)
                    .text_color(theme.sidebar_background)
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.accent.opacity(0.8)))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        view.update(cx, |this, cx| this.handle_submit(window, cx));
                    })
                    .child("Send"),
            )
    }
}
