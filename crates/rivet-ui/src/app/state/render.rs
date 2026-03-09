use super::AppView;
use crate::components::remote_image::avatar_fallback_label;
use crate::security::verification::SasVerificationPage;
use crate::theme::onedark::OneDarkThemeExt;
use gpui::prelude::FluentBuilder;
use gpui::*;

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.active_timeline_model.is_some() && self.active_chat_view.is_none() {
            let model = self.active_timeline_model.as_ref().unwrap().clone();
            let room_list_model = self.room_list_model.as_ref().unwrap().clone();
            let chat_view = cx.new(|cx| {
                crate::components::chat::ChatView::new(room_list_model, model, window, cx)
            });
            self.active_chat_view = Some(chat_view);
        }
        let theme = cx.onedark_theme();

        if !self.is_logged_in {
            return div().size_full().child(self.login_view.clone());
        }

        if self.verification_gate_active {
            if let Some(vm) = &self.verification_model {
                return div()
                    .size_full()
                    .child(cx.new(|_| SasVerificationPage::new(vm.clone())));
            }

            return div()
                .size_full()
                .bg(theme.background)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    div()
                        .w_16()
                        .h_16()
                        .rounded_full()
                        .bg(theme.sidebar_item_active)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.accent)
                                .child("R"),
                        ),
                )
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text)
                        .child("Verifying this session..."),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child("Secure your keys before loading the main interface."),
                );
        }

        if !self.is_initial_sync_complete(cx) {
            return div()
                .size_full()
                .bg(theme.background)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    div()
                        .w_16()
                        .h_16()
                        .rounded_full()
                        .bg(theme.sidebar_item_active)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.accent)
                                .child("R"),
                        ),
                )
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text)
                        .child("Syncing your data..."),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child(format!("Status: {}", self.sync_status)),
                );
        }

        div()
            .relative()
            .size_full()
            .child(
                div()
                    .flex()
                    .size_full()
                    .bg(theme.background)
                    .text_color(theme.text)
                    .child(self.sidebar.clone())
                    .child(if let Some(vm) = &self.verification_model {
                        cx.new(|_| SasVerificationPage::new(vm.clone()))
                            .into_any_element()
                    } else if let Some(chat_view) = &self.active_chat_view {
                        chat_view.clone().into_any_element()
                    } else if self.active_room_id.is_some() {
                        div()
                            .flex_1()
                            .bg(theme.background)
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.text_muted)
                                            .child("Loading messages..."),
                                    )
                                    .into_any_element(),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_4()
                            .child(
                                div()
                                    .w_24()
                                    .h_24()
                                    .bg(theme.sidebar_item_active)
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        if let Some(url) = self.sidebar.read(cx).avatar_url.clone()
                                        {
                                            let fallback = avatar_fallback_label(
                                                &self.sidebar.read(cx).display_name,
                                                &self.sidebar.read(cx).user_id,
                                            );
                                            crate::components::remote_image::RemoteImage::new(
                                                url.clone(),
                                            )
                                            .size(px(96.0))
                                            .avatar()
                                            .fallback_text(fallback)
                                            .into_any_element()
                                        } else {
                                            div()
                                                .w_24()
                                                .h_24()
                                                .rounded_full()
                                                .bg(theme.accent)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    div()
                                                        .text_color(theme.sidebar_background)
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(
                                                            self.sidebar
                                                                .read(cx)
                                                                .display_name
                                                                .chars()
                                                                .next()
                                                                .unwrap_or('U')
                                                                .to_string()
                                                                .to_uppercase(),
                                                        )
                                                        .into_any_element(),
                                                )
                                                .into_any_element()
                                        },
                                    ),
                            )
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text_muted)
                                    .child("Select a room to start chatting"),
                            )
                            .into_any_element()
                    }),
            )
            .when(self.is_settings_open, |el| {
                el.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .child(self.settings_view.clone()),
                )
            })
    }
}
