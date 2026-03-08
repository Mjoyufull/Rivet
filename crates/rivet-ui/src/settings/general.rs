use super::{SettingsEvent, SettingsView};
use crate::theme::onedark::OneDarkThemeExt;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::button::Button;
use gpui_component::button::ButtonVariants;
use gpui_component::input::Input;

impl SettingsView {
    pub(super) fn render_general_settings(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let view = cx.entity().clone();
        let view_for_verify = view.clone();
        let show_rooms = self.show_rooms_in_home;
        let card_radius = self.element_radius;
        let inner_card_radius = px((f32::from(card_radius) - 1.0).max(0.0));

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
                    .bg(theme.border)
                    .corner_radii(Corners::all(card_radius))
                    .p(px(1.0))
                    .child(
                        div()
                            .p_4()
                            .bg(theme.background)
                            .corner_radii(Corners::all(inner_card_radius))
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
                    ),
            )
            .child(
                div()
                    .bg(theme.border)
                    .corner_radii(Corners::all(card_radius))
                    .p(px(1.0))
                    .child(
                        div()
                            .p_4()
                            .bg(theme.background)
                            .corner_radii(Corners::all(inner_card_radius))
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
                                        view_for_verify.update(cx, |_, cx| {
                                            cx.emit(SettingsEvent::VerifySession)
                                        });
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .bg(theme.border)
                    .corner_radii(Corners::all(card_radius))
                    .p(px(1.0))
                    .child(
                        div()
                            .p_4()
                            .bg(theme.background)
                            .corner_radii(Corners::all(inner_card_radius))
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
                                        .corner_radii(Corners::all(px(
                                            (f32::from(self.element_radius) * 0.75).clamp(0.0, 18.0),
                                        )))
                                        .text_color(rgb(0x98c379))
                                        .text_sm()
                                        .child("Session successfully recovered!"),
                                    Err(e) => div()
                                        .px_3()
                                        .py_2()
                                        .bg(rgba(0xe06c7520))
                                        .corner_radii(Corners::all(px(
                                            (f32::from(self.element_radius) * 0.75).clamp(0.0, 18.0),
                                        )))
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
                                            let key = view
                                                .read(cx)
                                                .recovery_input
                                                .read(cx)
                                                .text()
                                                .to_string();
                                            if !key.is_empty() {
                                                view.update(cx, |this, cx| {
                                                    this.recovery_status = None;
                                                    cx.emit(SettingsEvent::RecoverWithKey(key));
                                                });
                                            }
                                        }
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .bg(rgb(0xe06c75))
                    .corner_radii(Corners::all(card_radius))
                    .p(px(1.0))
                    .child(
                        div()
                            .p_4()
                            .bg(rgba(0xe06c7510))
                            .corner_radii(Corners::all(inner_card_radius))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(0xe06c75))
                                            .child("Danger Zone"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_color(theme.text_muted)
                                    .child(
                                        "Delete every local Rivet profile, session, and store from this machine. This logs you out everywhere in Rivet on this device.",
                                    ),
                            )
                            .child(
                                Button::new("delete-all-data-button")
                                    .label("Delete All Local Data")
                                    .icon(gpui_component::IconName::Delete)
                                    .danger()
                                    .on_click({
                                        let view = view.clone();
                                        move |_, _, cx| {
                                            view.update(cx, |_, cx| {
                                                cx.emit(SettingsEvent::DeleteAllData)
                                            });
                                        }
                                    }),
                            ),
                    ),
            )
    }
}
