use crate::models::appearance::{get_radius, set_radius};
use crate::theme::onedark::OneDarkThemeExt;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement;
use gpui_component::slider::{Slider, SliderEvent, SliderState, SliderValue};

#[derive(Debug, Clone)]
pub enum SettingsEvent {
    Close,
    Logout,
    VerifySession,
    RecoverWithKey(String),
    SetChatStyle(crate::models::timeline_model::ChatStyle),
    SetShowRoomsInHome(bool),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum SettingsTab {
    #[default]
    General,
    Appearance,
}

pub struct SettingsView {
    active_tab: SettingsTab,
    focus_handle: FocusHandle,
    show_rooms_in_home: bool,
    session_verified: bool,
    chat_style: crate::models::timeline_model::ChatStyle,
    avatar_radius: Pixels,
    avatar_radius_slider: Entity<SliderState>,
    recovery_input: Entity<InputState>,
    recovery_status: Option<Result<(), String>>,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SettingsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let avatar_radius = px(f32::from(get_radius(cx)).clamp(0.0, 22.0));
        let recovery_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Enter recovery key or passphrase...")
        });
        let avatar_radius_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(22.0)
                .step(1.0)
                .default_value(f32::from(avatar_radius))
        });

        cx.subscribe(
            &avatar_radius_slider,
            |this: &mut Self, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    let value = match value {
                        SliderValue::Single(v) => *v,
                        SliderValue::Range(_, end) => *end,
                    };
                    this.set_avatar_radius(px(value), cx);
                }
            },
        )
        .detach();

        Self {
            active_tab: SettingsTab::default(),
            focus_handle: cx.focus_handle(),
            show_rooms_in_home: false,
            session_verified: false,
            chat_style: crate::models::timeline_model::ChatStyle::default(),
            avatar_radius,
            avatar_radius_slider,
            recovery_input,
            recovery_status: None,
        }
    }

    pub fn set_recovery_status(
        &mut self,
        status: Option<Result<(), String>>,
        cx: &mut Context<Self>,
    ) {
        self.recovery_status = status;
        cx.notify();
    }

    pub fn set_show_rooms_in_home(&mut self, val: bool, cx: &mut Context<Self>) {
        self.show_rooms_in_home = val;
        cx.notify();
    }

    pub fn set_session_verified(&mut self, val: bool, cx: &mut Context<Self>) {
        self.session_verified = val;
        cx.notify();
    }

    pub fn set_avatar_radius(&mut self, radius: Pixels, cx: &mut Context<Self>) {
        let radius = px(f32::from(radius).clamp(0.0, 22.0));
        self.avatar_radius = radius;
        set_radius(radius, cx);
        cx.notify();
    }

    fn set_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let view = cx.entity().clone();
        let view_for_logout = view.clone();

        div()
            .w_64()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.sidebar_background)
            .border_r(px(1.0))
            .border_color(theme.border)
            .child(
                div().p_4().child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text)
                        .child("Settings"),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .child(self.render_tab_button(SettingsTab::General, "General", cx))
                    .child(self.render_tab_button(SettingsTab::Appearance, "Appearance", cx)),
            )
            .child(
                div()
                    .p_4()
                    .border_t(px(1.0))
                    .border_color(theme.border)
                    .child(
                        Button::new("logout-button")
                            .label("Logout")
                            .icon(gpui_component::IconName::ExternalLink)
                            .danger()
                            .on_click(move |_, _, cx| {
                                view_for_logout.update(cx, |_, cx| cx.emit(SettingsEvent::Logout));
                            }),
                    ),
            )
    }

    fn render_tab_button(
        &self,
        tab: SettingsTab,
        label: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let is_active = self.active_tab == tab;
        let view = cx.entity().clone();
        let hover_bg = theme.sidebar_item_hover;
        let active_bg = theme.sidebar_item_active;
        let accent_color = theme.accent;
        let text_color = theme.text;
        let label_owned = label.to_string();

        div()
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .bg(if is_active {
                active_bg
            } else {
                gpui::transparent_black()
            })
            .text_color(if is_active { accent_color } else { text_color })
            .child(label_owned)
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                view.update(cx, |this, cx| this.set_tab(tab, cx));
            })
            .when(!is_active, move |el: Div| {
                el.hover(move |s: StyleRefinement| s.bg(hover_bg))
            })
    }

    fn render_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();

        div()
            .flex_1()
            .h_full()
            .bg(theme.background)
            .p_8()
            .overflow_y_scrollbar()
            .child(match self.active_tab {
                SettingsTab::General => self.render_general_settings(cx).into_any_element(),
                SettingsTab::Appearance => self.render_appearance_settings(cx).into_any_element(),
            })
    }

    fn render_general_settings(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let view = cx.entity().clone();
        let view_for_verify = view.clone();
        let show_rooms = self.show_rooms_in_home;

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text)
                    .child("General"),
            )
            .child(
                div()
                    .p_4()
                    .rounded_lg()
                    .border(px(1.0))
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("Sidebar Preferences"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w_10()
                                    .h_5()
                                    .rounded_full()
                                    .bg(if show_rooms {
                                        theme.accent
                                    } else {
                                        theme.sidebar_item_active
                                    })
                                    .flex()
                                    .items_center()
                                    .p_0p5()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, {
                                        let view = view.clone();
                                        move |_, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.show_rooms_in_home = !show_rooms;
                                                cx.emit(SettingsEvent::SetShowRoomsInHome(
                                                    !show_rooms,
                                                ));
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .child(
                                        div()
                                            .size_4()
                                            .rounded_full()
                                            .bg(theme.text)
                                            .when(show_rooms, |el| el.ml_auto()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_color(theme.text)
                                    .child("Show rooms alongside direct messages in Home"),
                            ),
                    ),
            )
            // Session verification button
            .child(
                div()
                    .p_4()
                    .rounded_lg()
                    .border(px(1.0))
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child("Session Verification"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_full()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .bg(if self.session_verified {
                                        rgba(0x98c37930)
                                    } else {
                                        rgba(0xe5c07b28)
                                    })
                                    .text_color(if self.session_verified {
                                        rgb(0x98c379)
                                    } else {
                                        rgb(0xe5c07b)
                                    })
                                    .child(if self.session_verified {
                                        "Verified"
                                    } else {
                                        "Unverified"
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_color(theme.text_muted)
                            .child("Verify this session to access encrypted messages."),
                    )
                    .child(
                        Button::new("verify-session-button")
                            .label("Verify Session")
                            .icon(gpui_component::IconName::CircleCheck)
                            .on_click(move |_, _, cx| {
                                view_for_verify
                                    .update(cx, |_, cx| cx.emit(SettingsEvent::VerifySession));
                            }),
                    ),
            )
            // Recovery Key section
            .child(
                div()
                    .p_4()
                    .rounded_lg()
                    .border(px(1.0))
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("Restore Key Verification"),
                    )
                    .child(
                        div()
                            .text_color(theme.text_muted)
                            .child("Use your recovery key or passphrase to verify this session."),
                    )
                    .child(Input::new(&self.recovery_input))
                    .children(self.recovery_status.as_ref().map(|res| {
                        match res {
                            Ok(_) => div()
                                .px_3()
                                .py_2()
                                .bg(rgba(0x98c37920))
                                .rounded_md()
                                .text_color(rgb(0x98c379))
                                .text_sm()
                                .child("Session successfully recovered!"),
                            Err(e) => div()
                                .px_3()
                                .py_2()
                                .bg(rgba(0xe06c7520))
                                .rounded_md()
                                .text_color(rgb(0xe06c75))
                                .text_sm()
                                .child(format!("Recovery failed: {}", e)),
                        }
                    }))
                    .child(
                        Button::new("recover-key-button")
                            .label("Verify with Key")
                            .icon(gpui_component::IconName::Globe)
                            .on_click({
                                let view = view.clone();
                                move |_, _, cx| {
                                    let key =
                                        view.read(cx).recovery_input.read(cx).text().to_string();
                                    if !key.is_empty() {
                                        view.update(cx, |this, cx| {
                                            this.recovery_status = None;
                                            cx.emit(SettingsEvent::RecoverWithKey(key));
                                        });
                                    }
                                }
                            }),
                    ),
            )
    }

    fn render_appearance_settings(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text)
                    .child("Appearance"),
            )
            .child(
                div()
                    .p_4()
                    .rounded_lg()
                    .border(px(1.0))
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("Avatar Radius"),
                    )
                    .child(div().text_color(theme.text_muted).child(
                        "Adjust avatar corner radius from square (0) to full circle (22px).",
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().text_xs().text_color(theme.text_muted).child("Square"))
                            .child(
                                Slider::new(&self.avatar_radius_slider)
                                    .horizontal()
                                    .flex_1(),
                            )
                            .child(div().text_xs().text_color(theme.text_muted).child("Circle")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(format!("Current: {:.0}px", f32::from(self.avatar_radius))),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(self.render_style_button(
                                crate::models::timeline_model::ChatStyle::Modern,
                                "Modern",
                                cx,
                            ))
                            .child(self.render_style_button(
                                crate::models::timeline_model::ChatStyle::Bubble,
                                "Bubble",
                                cx,
                            )),
                    ),
            )
    }

    fn render_style_button(
        &self,
        style: crate::models::timeline_model::ChatStyle,
        label: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let is_active = self.chat_style == style;
        let view = cx.entity().clone();
        let label_owned = label.to_string();

        div()
            .px_4()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .border(px(1.0))
            .border_color(if is_active {
                theme.accent
            } else {
                theme.border
            })
            .bg(if is_active {
                theme.accent.opacity(0.1)
            } else {
                gpui::transparent_black()
            })
            .text_color(if is_active { theme.accent } else { theme.text })
            .child(label_owned)
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                view.update(cx, |this, cx| {
                    this.chat_style = style;
                    cx.emit(SettingsEvent::SetChatStyle(style));
                    cx.notify();
                });
            })
    }
}

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
