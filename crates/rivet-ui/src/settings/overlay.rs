use super::{SettingsEvent, SettingsView};
use crate::theme::onedark::OneDarkThemeExt;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let view = cx.entity().clone();
        let view_for_close = view.clone();
        let view_for_backdrop = view.clone();

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(hsla(0.0, 0.0, 0.0, 0.5))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        view_for_backdrop.update(cx, |_, cx| cx.emit(SettingsEvent::Close));
                    }),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w(px(800.0))
                            .h(px(600.0))
                            .bg(theme.background)
                            .rounded_xl()
                            .shadow_xl()
                            .border(px(1.0))
                            .border_color(theme.border)
                            .flex()
                            .overflow_hidden()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .child(self.render_sidebar(cx))
                            .child(self.render_content(cx))
                            .child(
                                div().absolute().top_4().right_4().child(
                                    Button::new("close-settings")
                                        .icon(gpui_component::IconName::Close)
                                        .ghost()
                                        .on_click(move |_, _, cx| {
                                            view_for_close
                                                .update(cx, |_, cx| cx.emit(SettingsEvent::Close));
                                        }),
                                ),
                            ),
                    ),
            )
    }
}
