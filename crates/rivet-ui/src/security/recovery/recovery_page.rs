use gpui::*;

/// Pages in the recovery flow navigation stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecoveryFlowPage {
    /// Enter recovery key/passphrase to recover.
    #[default]
    Recover,
    /// Reset recovery (and optionally cross-signing/backup).
    Reset,
    /// Enable recovery (create new key).
    Enable,
    /// Successfully enabled recovery (show key).
    Success,
    /// Recovery incomplete (warning).
    Incomplete,
}

/// Recovery setup page component.
pub struct RecoveryPage {
    current_page: RecoveryFlowPage,
    recovery_key: SharedString,
    passphrase_input: SharedString,
    generated_key: Option<SharedString>,
    is_loading: bool,
    reset_identity: bool,
    reset_backup: bool,
}

impl RecoveryPage {
    pub fn new() -> Self {
        Self {
            current_page: RecoveryFlowPage::Recover,
            recovery_key: SharedString::default(),
            passphrase_input: SharedString::default(),
            generated_key: None,
            is_loading: false,
            reset_identity: false,
            reset_backup: false,
        }
    }

    /// Set initial page based on current recovery state.
    pub fn set_initial_page(&mut self, page: RecoveryFlowPage) {
        self.current_page = page;
    }

    /// Navigate to a page.
    pub fn navigate_to(&mut self, page: RecoveryFlowPage) {
        self.current_page = page;
    }

    /// Set the generated recovery key (after enabling).
    pub fn set_generated_key(&mut self, key: Option<SharedString>) {
        self.generated_key = key;
    }
}

impl Render for RecoveryPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page_content: AnyElement = match self.current_page {
            RecoveryFlowPage::Recover => self.render_recover(cx).into_any_element(),
            RecoveryFlowPage::Reset => self.render_reset(cx).into_any_element(),
            RecoveryFlowPage::Enable => self.render_enable(cx).into_any_element(),
            RecoveryFlowPage::Success => self.render_success(cx).into_any_element(),
            RecoveryFlowPage::Incomplete => self.render_incomplete(cx).into_any_element(),
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(rgb(0x1e1e2e))
            .child(page_content)
    }
}

impl RecoveryPage {
    fn render_recover(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .text_color(rgb(0xcdd6f4))
                    .child("Recover Your Account")
            )
            .child(
                div()
                    .text_color(rgb(0xa6adc8))
                    .text_center()
                    .max_w_80()
                    .child("Enter your recovery key or passphrase to restore access to your encrypted messages.")
            )
            .child(
                div()
                    .px_4()
                    .py_2()
                    .bg(rgb(0x313244))
                    .text_color(rgb(0xcdd6f4))
                    .rounded_md()
                    .min_w_64()
                    .child("Recovery Key")
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .bg(rgb(0x45475a))
                            .text_color(rgb(0xcdd6f4))
                            .rounded_lg()
                            .cursor_pointer()
                            .child("Reset Instead")
                    )
                    .child(
                        div()
                            .px_6()
                            .py_2()
                            .bg(rgb(0x89b4fa))
                            .text_color(rgb(0x1e1e2e))
                            .rounded_lg()
                            .cursor_pointer()
                            .child("Recover")
                    )
            )
    }

    fn render_reset(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .text_color(rgb(0xcdd6f4))
                    .child("Reset Account Recovery"),
            )
            .child(
                div()
                    .text_color(rgb(0xf9e2af)) // Warning yellow
                    .text_center()
                    .max_w_80()
                    .child("⚠️ You may lose access to past encrypted messages if you reset."),
            )
            // Toggle: Reset crypto identity
            .child(self.render_toggle(
                "Reset crypto identity",
                self.reset_identity,
                "Invalidates the verifications of all users and sessions",
            ))
            // Toggle: Reset backup
            .child(self.render_toggle(
                "Reset backup",
                self.reset_backup,
                "You might not be able to read your past encrypted messages anymore",
            ))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .bg(rgb(0x45475a))
                            .text_color(rgb(0xcdd6f4))
                            .rounded_lg()
                            .cursor_pointer()
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .px_6()
                            .py_2()
                            .bg(rgb(0xf38ba8))
                            .text_color(rgb(0x1e1e2e))
                            .rounded_lg()
                            .cursor_pointer()
                            .child("Reset"),
                    ),
            )
    }

    fn render_toggle(&self, label: &str, checked: bool, description: &str) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_4()
            .px_4()
            .py_3()
            .bg(rgb(0x313244))
            .rounded_lg()
            .min_w_80()
            .child(div().w_12().h_6().rounded_full().bg(if checked {
                rgb(0x89b4fa)
            } else {
                rgb(0x45475a)
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_color(rgb(0xcdd6f4)).child(label.to_string()))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0xa6adc8))
                            .child(description.to_string()),
                    ),
            )
    }

    fn render_enable(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .text_color(rgb(0xcdd6f4))
                    .child("Enable Account Recovery")
            )
            .child(
                div()
                    .text_color(rgb(0xa6adc8))
                    .text_center()
                    .max_w_80()
                    .child("Create a recovery key or set a passphrase to secure your encrypted messages.")
            )
            .child(
                div()
                    .px_4()
                    .py_2()
                    .bg(rgb(0x313244))
                    .text_color(rgb(0x6c7086))
                    .rounded_md()
                    .min_w_64()
                    .child("Optional passphrase...")
            )
            .child(
                div()
                    .px_6()
                    .py_2()
                    .bg(rgb(0xa6e3a1))
                    .text_color(rgb(0x1e1e2e))
                    .rounded_lg()
                    .cursor_pointer()
                    .child("Enable Recovery")
            )
    }

    fn render_success(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let key_display = if let Some(key) = &self.generated_key {
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_color(rgb(0xa6adc8))
                        .text_center()
                        .max_w_80()
                        .child("Make sure to store this recovery key in a safe place."),
                )
                .child(
                    div()
                        .px_4()
                        .py_3()
                        .bg(rgb(0x313244))
                        .text_color(rgb(0xcdd6f4))
                        .rounded_md()
                        .font_family("monospace")
                        .child(key.clone()),
                )
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(rgb(0x45475a))
                        .text_color(rgb(0xcdd6f4))
                        .rounded_lg()
                        .cursor_pointer()
                        .child("Copy to Clipboard"),
                )
        } else {
            div()
                .text_color(rgb(0xa6adc8))
                .text_center()
                .max_w_80()
                .child("Make sure to remember your passphrase or store it in a safe place.")
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            .child(div().text_3xl().text_color(rgb(0xa6e3a1)).child("✓"))
            .child(
                div()
                    .text_2xl()
                    .text_color(rgb(0xcdd6f4))
                    .child("Recovery Enabled"),
            )
            .child(key_display)
            .child(
                div()
                    .px_6()
                    .py_2()
                    .bg(rgb(0x89b4fa))
                    .text_color(rgb(0x1e1e2e))
                    .rounded_lg()
                    .cursor_pointer()
                    .child("Done"),
            )
    }

    fn render_incomplete(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            .child(
                div()
                    .text_3xl()
                    .text_color(rgb(0xf9e2af))
                    .child("⚠️")
            )
            .child(
                div()
                    .text_2xl()
                    .text_color(rgb(0xcdd6f4))
                    .child("Recovery Incomplete")
            )
            .child(
                div()
                    .text_color(rgb(0xa6adc8))
                    .text_center()
                    .max_w_80()
                    .child("Your recovery data may not contain all the necessary information. Some features might be limited.")
            )
            .child(
                div()
                    .px_6()
                    .py_2()
                    .bg(rgb(0x89b4fa))
                    .text_color(rgb(0x1e1e2e))
                    .rounded_lg()
                    .cursor_pointer()
                    .child("Continue Anyway")
            )
    }
}
