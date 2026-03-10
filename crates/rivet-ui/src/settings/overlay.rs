use super::{SettingsEvent, SettingsView};
use crate::theme::onedark::OneDarkThemeExt;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::button::{Button, ButtonVariants};

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let view = cx.entity().clone();
        let view_for_close = view.clone();
        let view_for_backdrop = view.clone();
        let viewport = window.viewport_size();
        let viewport_width = f32::from(viewport.width);
        let viewport_height = f32::from(viewport.height);
        let modal_width = px((viewport_width - 48.0)
            .max(320.0)
            .min(960.0)
            .min((viewport_width - 16.0).max(240.0)));
        let modal_height = px((viewport_height - 56.0)
            .max(360.0)
            .min(760.0)
            .min((viewport_height - 16.0).max(280.0)));
        let panel_radius = px((f32::from(self.element_radius) * 1.2).clamp(0.0, 24.0));
        let inner_panel_radius = px((f32::from(panel_radius) - 1.0).max(0.0));

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
                    .on_scroll_wheel(|_, _, cx| {
                        cx.stop_propagation();
                    })
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
                    .on_scroll_wheel(|_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        div()
                            .relative()
                            .w(modal_width)
                            .h(modal_height)
                            .min_w(px(280.0))
                            .min_h(px(320.0))
                            .max_w(px(960.0))
                            .max_h(px(760.0))
                            .bg(theme.border)
                            .corner_radii(Corners::all(panel_radius))
                            .shadow_xl()
                            .p(px(1.0))
                            .child(
                                div()
                                    .relative()
                                    .size_full()
                                    .bg(theme.background)
                                    .corner_radii(Corners::all(inner_panel_radius))
                                    .flex()
                                    .overflow_hidden()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| {
                                        cx.stop_propagation();
                                    })
                                    .on_scroll_wheel(|_, _, cx| {
                                        cx.stop_propagation();
                                    })
                                    .child(self.render_sidebar(modal_width, inner_panel_radius, cx))
                                    .child(self.render_content(modal_width, inner_panel_radius, cx))
                                    .child(
                                        div().absolute().top_4().right_4().child(
                                            Button::new("close-settings")
                                                .icon(gpui_component::IconName::Close)
                                                .ghost()
                                                .on_click(move |_, _, cx| {
                                                    view_for_close.update(cx, |_, cx| {
                                                        cx.emit(SettingsEvent::Close)
                                                    });
                                                }),
                                        ),
                                    ),
                            ),
                    ),
            )
    }
}
