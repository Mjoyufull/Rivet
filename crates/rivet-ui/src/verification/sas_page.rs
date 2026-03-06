use crate::models::verification::VerificationModel;
use crate::verification::VerificationState;
use gpui::*;

/// SAS verification page showing state-dependent UI.
///
/// Renders different views based on the verification state:
/// - Requested: Accept/Reject incoming request
/// - SasConfirm: 7 emoji in a 4+3 grid with Match/Mismatch buttons
/// - Done: Success message with Dismiss button
/// - Cancelled/Error: Failure reason with Dismiss button
/// - Other states: Loading/waiting indicator
pub struct SasVerificationPage {
    model: Entity<VerificationModel>,
}

impl SasVerificationPage {
    pub fn new(model: Entity<VerificationModel>) -> Self {
        Self { model }
    }
}

impl Render for SasVerificationPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let state = model.state();
        let is_self = model.is_self_verification();

        let title = match state {
            VerificationState::Requested => {
                if is_self {
                    "Verify This Session"
                } else {
                    "Verification Request"
                }
            }
            VerificationState::SasConfirm => {
                if is_self {
                    "Verify Session"
                } else {
                    "Verify User"
                }
            }
            VerificationState::Done => "Verification Complete",
            VerificationState::Cancelled | VerificationState::Error => "Verification Failed",
            _ => {
                if is_self {
                    "Verify Session"
                } else {
                    "Verification"
                }
            }
        };

        let content = match state {
            VerificationState::Requested => self.render_request_view(cx),
            VerificationState::Ready | VerificationState::Accepted | VerificationState::Created => {
                self.render_waiting_view(cx)
            }
            VerificationState::SasConfirm => self.render_emoji_view(cx),
            VerificationState::Done => self.render_done_view(cx),
            VerificationState::Cancelled | VerificationState::Error => {
                self.render_cancelled_view(cx)
            }
            _ => self.render_waiting_view(cx),
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_6()
            .bg(rgb(0x181a1f))
            // Title
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xabb2bf))
                    .child(title),
            )
            // Dynamic content
            .child(content)
    }
}

impl SasVerificationPage {
    /// Render the incoming request view with Accept/Reject buttons.
    fn render_request_view(&self, cx: &Context<Self>) -> AnyElement {
        let model = self.model.read(cx);
        let is_self = model.is_self_verification();
        let other = model.other_user_id().unwrap_or("Unknown").to_string();
        let we_started = model.request().map(|r| r.we_started()).unwrap_or(false);

        let instructions = if we_started {
            "Waiting for your other device to accept the verification request...".to_string()
        } else if is_self {
            "Another session is requesting verification. Accept to compare emoji.".to_string()
        } else {
            format!("{} wants to verify with you.", other)
        };

        let model_accept = self.model.clone();
        let model_reject = self.model.clone();

        let buttons = if we_started {
            // If we started it, we only have the option to cancel our request, not "accept" our own request
            div().flex().gap_4().mt_4().child(
                div()
                    .px_6()
                    .py_3()
                    .bg(rgb(0xe06c75))
                    .text_color(rgb(0x181a1f))
                    .rounded_md()
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .hover(|s| s.bg(hsla(0.0, 0.8, 0.7, 1.0)))
                    .child("Cancel")
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        model_reject.update(cx, |m, cx| m.cancel(cx));
                    }),
            )
        } else {
            // Incoming request has accept/reject
            div()
                .flex()
                .gap_4()
                .mt_4()
                .child(
                    div()
                        .px_6()
                        .py_3()
                        .bg(rgb(0xe06c75))
                        .text_color(rgb(0x181a1f))
                        .rounded_md()
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(hsla(0.0, 0.8, 0.7, 1.0)))
                        .child("Reject")
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            model_reject.update(cx, |m, cx| m.cancel(cx));
                        }),
                )
                .child(
                    div()
                        .px_6()
                        .py_3()
                        .bg(rgb(0x98c379))
                        .text_color(rgb(0x181a1f))
                        .rounded_md()
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(hsla(100.0, 0.8, 0.7, 1.0)))
                        .child("Accept")
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            model_accept.update(cx, |m, cx| m.accept_and_start_sas(cx));
                        }),
                )
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            // Instructions
            .child(
                div()
                    .text_color(rgb(0x5c6370))
                    .text_center()
                    .max_w_96()
                    .child(instructions),
            )
            // Buttons
            .child(buttons)
            .into_any_element()
    }

    /// Render the waiting/loading view.
    fn render_waiting_view(&self, cx: &Context<Self>) -> AnyElement {
        let model = self.model.read(cx);
        let state = model.state();
        let desc = state.description();
        let model_start = self.model.clone();
        let model_cancel = self.model.clone();

        let content = if state == VerificationState::Ready {
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_6()
                .child(
                    div()
                        .text_color(rgb(0x98c379))
                        .text_center()
                        .child("The other device is ready."),
                )
                .child(
                    div()
                        .text_color(rgb(0x5c6370))
                        .text_center()
                        .child("Click 'Start' to begin the emoji comparison."),
                )
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .child(
                            div()
                                .px_6()
                                .py_3()
                                .bg(rgb(0xe06c75))
                                .text_color(rgb(0x181a1f))
                                .rounded_md()
                                .font_weight(FontWeight::BOLD)
                                .cursor_pointer()
                                .hover(|s| s.bg(hsla(0.0, 0.8, 0.7, 1.0)))
                                .child("Cancel")
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    model_cancel.update(cx, |m, cx| m.cancel(cx));
                                }),
                        )
                        .child(
                            div()
                                .px_6()
                                .py_3()
                                .bg(rgb(0x98c379))
                                .text_color(rgb(0x181a1f))
                                .rounded_md()
                                .font_weight(FontWeight::BOLD)
                                .cursor_pointer()
                                .hover(|s| s.bg(hsla(100.0, 0.8, 0.7, 1.0)))
                                .child("Start")
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    model_start.update(cx, |m, cx| m.start_sas(cx));
                                }),
                        ),
                )
        } else {
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_4()
                .child(div().text_color(rgb(0x5c6370)).text_center().child(desc))
                .child(
                    div()
                        .text_color(rgb(0x5c6370))
                        .child("Waiting for the other device..."),
                )
                .child(
                    div()
                        .mt_4()
                        .px_6()
                        .py_3()
                        .bg(rgb(0xe06c75))
                        .text_color(rgb(0x181a1f))
                        .rounded_md()
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(hsla(0.0, 0.8, 0.7, 1.0)))
                        .child("Cancel")
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            model_cancel.update(cx, |m, cx| m.cancel(cx));
                        }),
                )
        };

        content.into_any_element()
    }

    /// Render the emoji comparison view with Match/Mismatch buttons.
    fn render_emoji_view(&self, cx: &Context<Self>) -> AnyElement {
        let model = self.model.read(cx);
        let is_self = model.is_self_verification();

        let instructions = if is_self {
            "Check if the same emoji appear in the same order on the other device."
        } else if let Some(_other) = model.other_user_id() {
            // We'll use a static string for the general case
            "Compare the emoji with the other user's device."
        } else {
            "Compare emoji with the other user."
        };

        let model_confirm = self.model.clone();
        let model_mismatch = self.model.clone();

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            // Instructions
            .child(
                div()
                    .text_color(rgb(0x5c6370))
                    .text_center()
                    .max_w_96()
                    .child(instructions),
            )
            // Emoji grid: row 1 (4 emoji)
            .child(self.render_emoji_row(0, 4, cx))
            // Emoji grid: row 2 (3 emoji)
            .child(self.render_emoji_row(4, 3, cx))
            // Buttons
            .child(
                div()
                    .flex()
                    .gap_4()
                    .mt_6()
                    .child(
                        div()
                            .px_6()
                            .py_3()
                            .bg(rgb(0xe06c75))
                            .text_color(rgb(0x181a1f))
                            .rounded_md()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .hover(|s| s.bg(hsla(0.0, 0.8, 0.7, 1.0)))
                            .child("They Don't Match")
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                model_mismatch.update(cx, |m, cx| m.mismatch(cx));
                            }),
                    )
                    .child(
                        div()
                            .px_6()
                            .py_3()
                            .bg(rgb(0x98c379))
                            .text_color(rgb(0x181a1f))
                            .rounded_md()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .hover(|s| s.bg(hsla(100.0, 0.8, 0.7, 1.0)))
                            .child("They Match")
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                model_confirm.update(cx, |m, cx| m.confirm(cx));
                            }),
                    ),
            )
            .into_any_element()
    }

    /// Render a row of emoji from the SDK's Emoji array.
    fn render_emoji_row(&self, start: usize, count: usize, cx: &Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let emojis = model.emojis();

        let emoji_widgets: Vec<_> = (start..start + count)
            .filter_map(|i| {
                let emoji = emojis?.get(i)?;
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .p_3()
                        .child(div().text_3xl().child(emoji.symbol.to_string()))
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0xa6adc8))
                                .child(emoji.description.to_string()),
                        ),
                )
            })
            .collect();

        div()
            .flex()
            .justify_center()
            .gap_4()
            .children(emoji_widgets)
    }

    /// Render the success view.
    fn render_done_view(&self, _cx: &Context<Self>) -> AnyElement {
        let model_dismiss = self.model.clone();

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            .child(div().text_3xl().child("✅"))
            .child(
                div()
                    .text_color(rgb(0x98c379))
                    .text_center()
                    .child("Session verified successfully!"),
            )
            .child(
                div()
                    .px_6()
                    .py_3()
                    .bg(rgb(0x61afef))
                    .text_color(rgb(0x181a1f))
                    .rounded_md()
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .hover(|s| s.bg(hsla(210.0, 0.8, 0.7, 1.0)))
                    .child("Done")
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        model_dismiss.update(cx, |m, cx| m.dismiss(cx));
                    }),
            )
            .into_any_element()
    }

    /// Render the cancelled/error view.
    fn render_cancelled_view(&self, cx: &Context<Self>) -> AnyElement {
        let model = self.model.read(cx);
        let reason = model
            .cancel_reason()
            .unwrap_or("Unknown reason")
            .to_string();
        let model_dismiss = self.model.clone();

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            .child(div().text_3xl().child("❌"))
            .child(div().text_color(rgb(0xe06c75)).text_center().child(reason))
            .child(
                div()
                    .px_6()
                    .py_3()
                    .bg(rgb(0x61afef))
                    .text_color(rgb(0x181a1f))
                    .rounded_md()
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .hover(|s| s.bg(hsla(210.0, 0.8, 0.7, 1.0)))
                    .child("Dismiss")
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        model_dismiss.update(cx, |m, cx| m.dismiss(cx));
                    }),
            )
            .into_any_element()
    }
}
