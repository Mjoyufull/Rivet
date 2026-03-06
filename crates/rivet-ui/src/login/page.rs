use gpui::*;

/// Pages in the login flow navigation stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoginFlowPage {
    /// Welcome screen with "Log In" button.
    #[default]
    Greeter,
    /// Server URL input with auto-discovery.
    Homeserver,
    /// Choose login method (SSO, password).
    Method,
    /// Waiting for SSO browser redirect.
    InBrowser,
    /// Crypto identity setup / session verification.
    SessionSetup,
    /// Login completed successfully.
    Completed,
}

/// The main login page component.
pub struct LoginPage {
    current_page: LoginFlowPage,
    homeserver_url: SharedString,
    is_loading: bool,
    error_message: Option<SharedString>,
}

impl LoginPage {
    pub fn new() -> Self {
        Self {
            current_page: LoginFlowPage::Greeter,
            homeserver_url: SharedString::from("matrix.org"),
            is_loading: false,
            error_message: None,
        }
    }

    /// Navigate to a specific page.
    pub fn navigate_to(&mut self, page: LoginFlowPage, _cx: &mut Context<Self>) {
        self.current_page = page;
        self.error_message = None;
    }

    /// Set homeserver URL.
    pub fn set_homeserver(&mut self, url: SharedString) {
        self.homeserver_url = url;
    }

    /// Get current page.
    pub fn current_page(&self) -> LoginFlowPage {
        self.current_page
    }
}

impl Render for LoginPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page_content: AnyElement = match self.current_page {
            LoginFlowPage::Greeter => self.render_greeter(cx).into_any_element(),
            LoginFlowPage::Homeserver => self.render_homeserver(cx).into_any_element(),
            LoginFlowPage::Method => self.render_method(cx).into_any_element(),
            LoginFlowPage::InBrowser => self.render_in_browser(cx).into_any_element(),
            LoginFlowPage::SessionSetup => self.render_session_setup(cx).into_any_element(),
            LoginFlowPage::Completed => self.render_completed(cx).into_any_element(),
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(rgb(0x181a1f)) // Sidebar darker bg for login
            .child(page_content)
    }
}

impl LoginPage {
    fn render_greeter(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xabb2bf))
                    .child("Rivet"),
            )
            .child(
                div()
                    .text_lg()
                    .text_color(rgb(0x5c6370))
                    .child("The premium Matrix experience"),
            )
            .child(
                div()
                    .px_10()
                    .py_3()
                    .bg(rgb(0x61afef))
                    .text_color(rgb(0x181a1f))
                    .rounded_md()
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .hover(|s| s.bg(hsla(210.0, 0.8, 0.7, 1.0)))
                    .child("Get Started"),
            )
    }

    fn render_homeserver(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .text_color(rgb(0xcdd6f4))
                    .child("Select a Homeserver"),
            )
            .child(
                div()
                    .px_4()
                    .py_2()
                    .bg(rgb(0x313244))
                    .text_color(rgb(0xcdd6f4))
                    .rounded_md()
                    .min_w_64()
                    .child(self.homeserver_url.clone()),
            )
            .child(
                div()
                    .px_6()
                    .py_2()
                    .bg(rgb(0x89b4fa))
                    .text_color(rgb(0x1e1e2e))
                    .rounded_lg()
                    .cursor_pointer()
                    .child("Continue"),
            )
    }

    fn render_method(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .text_color(rgb(0xcdd6f4))
                    .child("Choose Login Method"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(self.render_sso_button("Continue with SSO"))
                    .child(self.render_sso_button("Sign in with password")),
            )
    }

    fn render_sso_button(&self, label: &str) -> impl IntoElement {
        div()
            .px_6()
            .py_3()
            .bg(rgb(0x282c33))
            .text_color(rgb(0xabb2bf))
            .rounded_md()
            .border_1()
            .border_color(rgb(0x121417))
            .cursor_pointer()
            .min_w_64()
            .text_center()
            .hover(|s| s.bg(rgb(0x2c313a)).border_color(rgb(0x61afef)))
            .child(label.to_string())
    }

    fn render_in_browser(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .text_color(rgb(0xcdd6f4))
                    .child("Continue in Browser"),
            )
            .child(
                div()
                    .text_color(rgb(0xa6adc8))
                    .child("Complete the login in your browser, then return here."),
            )
            .child(div().w_12().h_12().rounded_full().bg(rgb(0x89b4fa)))
    }

    fn render_session_setup(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .text_color(rgb(0xcdd6f4))
                    .child("Verify Your Session")
            )
            .child(
                div()
                    .text_color(rgb(0xa6adc8))
                    .text_center()
                    .max_w_80()
                    .child("Verify this session to enable end-to-end encryption and access your encrypted messages.")
            )
    }

    fn render_completed(&self, _cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child("You're all set!"),
            )
            .child(
                div()
                    .text_color(rgb(0xa6adc8))
                    .child("Your session is verified and ready to use."),
            )
    }
}
