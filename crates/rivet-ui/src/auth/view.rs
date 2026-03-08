use crate::auth::sso::local_server::spawn_local_server;
use crate::auth::state::{LoginEvent, LoginFlowStep};
use crate::models::appearance::{element_radius_large, element_radius_small};
use crate::theme::onedark::OneDarkThemeExt;
use anyhow::Context as _;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use rivet_core::client::RivetClient;
use tokio::sync::oneshot;

pub struct LoginView {
    current_step: LoginFlowStep,
    homeserver_input: Entity<InputState>,
    username_input: Entity<InputState>,
    password_input: Entity<InputState>,
    is_loading: bool,
    error: Option<String>,
    sso_cancel: Option<oneshot::Sender<()>>,
    sso_generation: u64,
}

impl EventEmitter<LoginEvent> for LoginView {}

impl LoginView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let homeserver_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("https://matrix.org")
                .default_value("https://matrix.org")
        });
        let username_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("@user:matrix.org"));
        let password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("••••••••")
                .masked(true)
        });

        Self {
            current_step: LoginFlowStep::Welcome,
            homeserver_input,
            username_input,
            password_input,
            is_loading: false,
            error: None,
            sso_cancel: None,
            sso_generation: 0,
        }
    }

    fn handle_sso_login(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.sso_cancel.is_some() {
            return;
        }

        let homeserver = self.homeserver_input.read(cx).text().to_string();

        if homeserver.is_empty() {
            self.error = Some("Please enter a homeserver URL".to_string());
            cx.notify();
            return;
        }

        self.is_loading = true;
        self.error = None;
        self.current_step = LoginFlowStep::SsoInProgress;
        self.sso_generation = self.sso_generation.wrapping_add(1);
        let generation = self.sso_generation;
        let (cancel_tx, cancel_rx) = oneshot::channel();
        self.sso_cancel = Some(cancel_tx);
        cx.notify();

        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            enum SsoLoginResult {
                Success(RivetClient),
                Cancelled,
                Error(anyhow::Error),
            }

            let result = async {
                tracing::info!("Starting SSO login flow for homeserver: {}", homeserver);
                let mut handle = spawn_local_server().map_err(|e| anyhow::anyhow!("{}", e))?;
                let redirect_url = handle.redirect_url.clone();
                tracing::info!("Local server spawned at: {}", redirect_url);

                let client = RivetClient::new(&homeserver)
                    .await
                    .context("Failed to connect to homeserver")?;
                tracing::info!("Client created, requesting SSO URL");

                let sso_url = client
                    .get_sso_login_url(&redirect_url.to_string())
                    .await
                    .context("Failed to get SSO login URL")?;
                tracing::info!("Opening SSO URL: {}", sso_url);

                if let Err(e) = open::that(&sso_url) {
                    return Ok(SsoLoginResult::Error(anyhow::anyhow!(
                        "Failed to open browser: {}",
                        e
                    )));
                }

                tracing::info!("Waiting for SSO callback...");
                let callback_url = tokio::select! {
                    callback = handle.wait_for_callback() => {
                        callback.map_err(|e| anyhow::anyhow!("{}", e))?
                    }
                    _ = cancel_rx => {
                        tracing::info!("SSO login cancelled before callback");
                        return Ok(SsoLoginResult::Cancelled);
                    }
                };
                tracing::info!("Received SSO callback, completing login");

                let client = client.complete_sso_login(&callback_url.to_string()).await?;
                Ok(SsoLoginResult::Success(client))
            }
            .await;

            this.update(cx, |this, cx| {
                if this.sso_generation != generation {
                    return;
                }

                this.is_loading = false;
                this.sso_cancel = None;
                match result {
                    Ok(SsoLoginResult::Success(client)) => {
                        tracing::info!("SSO login successful, emitting Success event");
                        cx.emit(LoginEvent::Success(client));
                    }
                    Ok(SsoLoginResult::Cancelled) => {
                        this.current_step = LoginFlowStep::Welcome;
                        this.error = None;
                    }
                    Ok(SsoLoginResult::Error(e)) | Err(e) => {
                        tracing::error!("SSO login failed: {:?}", e);
                        this.current_step = LoginFlowStep::Welcome;
                        this.error = Some(e.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn handle_password_login(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let homeserver = self.homeserver_input.read(cx).text().to_string();
        let username = self.username_input.read(cx).text().to_string();
        let password = self.password_input.read(cx).text().to_string();

        if homeserver.is_empty() || username.is_empty() || password.is_empty() {
            self.error = Some("All fields are required".to_string());
            cx.notify();
            return;
        }

        self.is_loading = true;
        self.error = None;
        cx.notify();

        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let result = async {
                tracing::info!(
                    "Starting password login flow for homeserver: {}, user: {}",
                    homeserver,
                    username
                );
                let client = RivetClient::new(&homeserver)
                    .await
                    .context("Failed to create client")?;
                tracing::info!("Client created, attempting login");
                let client = client
                    .login(&username, &password)
                    .await
                    .context("Failed to login")?;
                Ok::<_, anyhow::Error>(client)
            }
            .await;

            this.update(cx, |this, cx| {
                this.is_loading = false;
                match result {
                    Ok(client) => {
                        tracing::info!("Password login successful, emitting Success event");
                        cx.emit(LoginEvent::Success(client));
                    }
                    Err(e) => {
                        tracing::error!("Password login failed: {:?}", e);
                        this.error = Some(e.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn show_password_form(&mut self, cx: &mut Context<Self>) {
        self.current_step = LoginFlowStep::PasswordForm;
        self.error = None;
        cx.notify();
    }

    fn go_back(&mut self, cx: &mut Context<Self>) {
        self.current_step = LoginFlowStep::Welcome;
        self.error = None;
        cx.notify();
    }

    fn cancel_sso_login(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = self.sso_cancel.take() {
            let _ = cancel.send(());
        }

        self.sso_generation = self.sso_generation.wrapping_add(1);
        self.is_loading = false;
        self.current_step = LoginFlowStep::Welcome;
        self.error = None;
        cx.notify();
    }

    fn render_welcome(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let view = cx.entity().clone();
        let password_view = cx.entity().clone();
        let panel_radius = element_radius_large(cx);
        let item_radius = element_radius_small(cx);

        div()
            .w_96()
            .p_8()
            .bg(theme.sidebar_background)
            .border(px(1.0))
            .border_color(theme.border)
            .corner_radii(Corners::all(panel_radius))
            .shadow_xl()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_3xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.accent)
                                    .child("Rivet"),
                            )
                            .child(
                                div()
                                    .text_color(theme.text_muted)
                                    .child("A modern Matrix client"),
                            ),
                    )
                    .child(self.render_field("HOMESERVER".into(), &self.homeserver_input, cx))
                    .children(self.error.as_ref().map(|e| {
                        div()
                            .px_3()
                            .py_2()
                            .bg(rgba(0xf38ba820))
                            .corner_radii(Corners::all(item_radius))
                            .text_color(rgb(0xf38ba8))
                            .text_sm()
                            .child(e.clone())
                    }))
                    .child(
                        div().w_full().child(
                            Button::new("sso-login")
                                .icon(gpui_component::IconName::Globe)
                                .label("Continue with SSO")
                                .loading(self.is_loading)
                                .on_click(move |_, window, cx| {
                                    view.update(cx, |this, cx| this.handle_sso_login(window, cx));
                                }),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(div().flex_1().h(px(1.0)).bg(theme.border))
                            .child(div().text_color(theme.text_muted).text_sm().child("or"))
                            .child(div().flex_1().h(px(1.0)).bg(theme.border)),
                    )
                    .child(
                        div().w_full().child(
                            Button::new("password-login")
                                .label("Sign in with password")
                                .on_click(move |_, _, cx| {
                                    password_view
                                        .update(cx, |this, cx| this.show_password_form(cx));
                                }),
                        ),
                    ),
            )
    }

    fn render_sso_in_progress(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let back_view = cx.entity().clone();
        let cancel_view = back_view.clone();
        let panel_radius = element_radius_large(cx);
        let item_radius = element_radius_small(cx);

        div()
            .w_96()
            .p_8()
            .bg(theme.sidebar_background)
            .border(px(1.0))
            .border_color(theme.border)
            .corner_radii(Corners::all(panel_radius))
            .shadow_xl()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_6()
                    .child(
                        div().w_full().flex().justify_start().child(
                            div()
                                .cursor_pointer()
                                .text_color(theme.text_muted)
                                .child("← Back")
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    back_view.update(cx, |this, cx| {
                                        this.cancel_sso_login(cx);
                                    });
                                }),
                        ),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text)
                            .child("Continue in Browser"),
                    )
                    .child(
                        div()
                            .text_color(theme.text_muted)
                            .text_center()
                            .child("Complete the login in your browser, then return here."),
                    )
                    .child(div().w_12().h_12().rounded_full().bg(theme.accent))
                    .child(
                        div().w_full().child(
                            Button::new("cancel-sso").label("Cancel SSO").on_click(
                                move |_, _, cx| {
                                    cancel_view.update(cx, |this, cx| {
                                        this.cancel_sso_login(cx);
                                    });
                                },
                            ),
                        ),
                    )
                    .children(self.error.as_ref().map(|e| {
                        div()
                            .px_3()
                            .py_2()
                            .bg(rgba(0xf38ba820))
                            .corner_radii(Corners::all(item_radius))
                            .text_color(rgb(0xf38ba8))
                            .text_sm()
                            .child(e.clone())
                    })),
            )
    }

    fn render_password_form(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.onedark_theme();
        let login_view = cx.entity().clone();
        let back_view = cx.entity().clone();
        let panel_radius = element_radius_large(cx);
        let item_radius = element_radius_small(cx);

        div()
            .w_96()
            .p_8()
            .bg(theme.sidebar_background)
            .border(px(1.0))
            .border_color(theme.border)
            .corner_radii(Corners::all(panel_radius))
            .shadow_xl()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .cursor_pointer()
                                    .text_color(theme.text_muted)
                                    .child("←")
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        back_view.update(cx, |this, cx| this.go_back(cx));
                                    }),
                            )
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text)
                                    .child("Sign In"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(self.render_field(
                                "HOMESERVER".into(),
                                &self.homeserver_input,
                                cx,
                            ))
                            .child(self.render_field("USERNAME".into(), &self.username_input, cx))
                            .child(self.render_field("PASSWORD".into(), &self.password_input, cx)),
                    )
                    .children(self.error.as_ref().map(|e| {
                        div()
                            .px_3()
                            .py_2()
                            .bg(rgba(0xf38ba820))
                            .corner_radii(Corners::all(item_radius))
                            .text_color(rgb(0xf38ba8))
                            .text_sm()
                            .child(e.clone())
                    }))
                    .child(
                        div().flex().justify_end().child(
                            Button::new("login-button")
                                .label("Login")
                                .loading(self.is_loading)
                                .on_click(move |_, window, cx| {
                                    login_view.update(cx, |this, cx| {
                                        this.handle_password_login(window, cx)
                                    });
                                }),
                        ),
                    ),
            )
    }

    fn render_field(
        &self,
        label: SharedString,
        state: &Entity<InputState>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.onedark_theme();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_muted)
                    .child(label),
            )
            .child(Input::new(state))
    }
}

impl Render for LoginView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg_color = cx.onedark_theme().background;

        let content: AnyElement = match self.current_step {
            LoginFlowStep::Welcome => self.render_welcome(cx).into_any_element(),
            LoginFlowStep::SsoInProgress => self.render_sso_in_progress(cx).into_any_element(),
            LoginFlowStep::PasswordForm => self.render_password_form(cx).into_any_element(),
        };

        div()
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .bg(bg_color)
            .child(content)
    }
}
