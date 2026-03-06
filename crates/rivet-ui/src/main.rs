mod components;
mod login;
mod models;
mod recovery;
mod theme;
mod verification;

use components::login::{LoginEvent, LoginView};
use components::settings::{SettingsEvent, SettingsView};
use components::sidebar::{Sidebar, SidebarEvent};
use gpui::Entity;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::Root;
use rivet_core::client::RivetClient;
use std::sync::Arc;
use theme::onedark::OneDarkThemeExt;

use crate::models::image_cache::ImageCache;
use crate::models::timeline_model::TimelineModel;
use crate::models::verification::VerificationEvent;
use components::chat::ChatView;
use gpui::AsyncApp;
use matrix_sdk::ruma::RoomId;
use matrix_sdk_ui::timeline::RoomExt;
use models::rooms_model::RoomListModel;
use models::verification::VerificationModel;
use verification::SasVerificationPage;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = runtime.enter();

    tracing::info!("Starting Rivet UI");

    Application::new().run(|cx| {
        gpui_component::init(cx);
        models::appearance::init(cx);
        cx.set_global(ImageCache::new());
        gpui_component::Theme::global_mut(cx).colors =
            theme::onedark::ONEDARK_THEME.to_theme_color();

        cx.open_window(WindowOptions::default(), |window, cx| {
            let sidebar = cx.new(|_| Sidebar::new(None, None));

            let login_view = cx.new(|cx| LoginView::new(window, cx));
            let settings_view = cx.new(|cx| SettingsView::new(window, cx));

            let main_view = cx.new(|cx| {
                let view = AppView {
                    sidebar,
                    login_view: login_view.clone(),
                    settings_view: settings_view.clone(),
                    is_logged_in: false,
                    is_settings_open: false,
                    client: None,
                    room_list_model: None,
                    active_timeline_model: None,
                    active_chat_view: None,
                    verification_model: None,
                    active_room_id: None,
                    sync_status: "Idle".to_string(),
                    session_verified: false,
                    is_recovering: false,
                    recovery_status: None,
                };

                cx.subscribe(
                    &login_view,
                    |this: &mut AppView, _, event: &LoginEvent, cx| match event {
                        LoginEvent::Success(client) => {
                            this.login(client.clone(), cx);
                        }
                    },
                )
                .detach();

                // Subscribe to settings events
                cx.subscribe(
                    &settings_view,
                    |this: &mut AppView, _, event: &SettingsEvent, cx| match event {
                        SettingsEvent::Close => this.close_settings(cx),
                        SettingsEvent::Logout => this.logout(cx),
                        SettingsEvent::VerifySession => this.start_self_verification(cx),
                        SettingsEvent::RecoverWithKey(key) => {
                            this.recover_with_key(key.clone(), cx)
                        }
                        SettingsEvent::SetChatStyle(style) => {
                            if let Some(timeline_model) = &this.active_timeline_model {
                                timeline_model.update(cx, |model, cx| {
                                    model.chat_style = *style;
                                    cx.notify();
                                });
                            }
                        }
                        SettingsEvent::SetShowRoomsInHome(val) => {
                            if let Some(room_model) = &this.room_list_model {
                                room_model.update(cx, |model, cx| {
                                    model.set_show_rooms_in_home(*val, cx);
                                });
                            }
                        }
                    },
                )
                .detach();

                view
            });

            // Try to restore session on startup
            let this = main_view.downgrade();
            let async_cx = cx.to_async();
            async_cx
                .clone()
                .spawn(move |_: &mut AsyncApp| {
                    let this = this.clone();
                    async move {
                        if let Ok(Some(client)) = RivetClient::restore().await {
                            async_cx.update(|cx| {
                                this.update(cx, |view, cx| {
                                    ImageCache::init(client.clone(), cx);
                                    view.login(client, cx);
                                })
                                .ok();
                            });
                        }
                    }
                })
                .detach();

            // Wrap in gpui_component::Root
            cx.new(|cx| Root::new(main_view, window, cx))
        })
        .unwrap();
    });
}

struct AppView {
    sidebar: Entity<Sidebar>,
    login_view: Entity<LoginView>,
    settings_view: Entity<SettingsView>,
    is_logged_in: bool,
    is_settings_open: bool,
    client: Option<RivetClient>,
    room_list_model: Option<Entity<RoomListModel>>,
    active_timeline_model: Option<Entity<TimelineModel>>,
    active_chat_view: Option<Entity<ChatView>>,
    verification_model: Option<Entity<VerificationModel>>,
    active_room_id: Option<String>,
    sync_status: String,
    session_verified: bool,
    is_recovering: bool,
    recovery_status: Option<Result<(), String>>,
}

impl AppView {
    fn is_initial_sync_complete(&self, cx: &App) -> bool {
        self.room_list_model
            .as_ref()
            .map(|model| !model.read(cx).is_loading)
            .unwrap_or(false)
    }

    fn set_session_verified(&mut self, verified: bool, cx: &mut Context<Self>) {
        if self.session_verified == verified {
            return;
        }

        self.session_verified = verified;
        self.settings_view.update(cx, |settings, cx| {
            settings.set_session_verified(verified, cx);
        });
        cx.notify();
    }

    fn login(&mut self, client: RivetClient, cx: &mut Context<Self>) {
        tracing::info!("Initializing UI for logged in user");
        self.client = Some(client.clone());
        ImageCache::init(client.clone(), cx);
        let room_list_model = RoomListModel::new(cx);
        RoomListModel::init(room_list_model.clone(), client.clone(), cx);
        self.room_list_model = Some(room_list_model.clone());

        // Observe room selection changes
        cx.observe(&room_list_model, |this: &mut AppView, model, cx| {
            let selected_id = model.read(cx).selected_room_id.clone();
            if this.active_room_id != selected_id {
                tracing::info!("Room selection changed to: {:?}", selected_id);
                this.active_room_id = selected_id.clone();
                this.active_timeline_model = None;
                this.active_chat_view = None;

                if let (Some(room_id_str), Some(client)) = (selected_id, &this.client) {
                    tracing::info!("Attempting to open room: {}", room_id_str);
                    if let Ok(room_id) = RoomId::parse(&room_id_str) {
                        if let Some(room) = client.client().get_room(&room_id) {
                            tracing::info!("Found room in client: {}", room_id);
                            let async_cx = cx.to_async();
                            let this_handle = cx.entity().downgrade();
                            let client = client.clone();
                            let room = room.clone();
                            async_cx.clone().spawn(move |_: &mut AsyncApp| async move {
                                tracing::info!("Building timeline for room: {}", room_id);
                                match room.timeline_builder().build().await {
                                    Ok(timeline) => {
                                        let timeline = Arc::new(timeline);
                                        let _ = async_cx.update(|cx| {
                                            let _ = this_handle.update(cx, |view, cx: &mut Context<AppView>| {
                                                tracing::info!("Initializing timeline model for room: {}", room_id);
                                                let homeserver_url = client.homeserver_url().to_string();
                                                let model = TimelineModel::new(room, timeline, homeserver_url, cx);
                                                TimelineModel::init(model.clone(), cx);
                                                view.active_timeline_model = Some(model);

                                                cx.notify();
                                            });
                                        });
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to build timeline for room {}: {:?}", room_id, e);
                                    }
                                }
                            }).detach();
                        } else {
                            tracing::warn!("Room not found in client: {}", room_id_str);
                        }
                    } else {
                        tracing::error!("Failed to parse room ID: {}", room_id_str);
                    }
                }
                cx.notify();
            }
        }).detach();

        self.is_logged_in = true;

        // Update sidebar with new model and fetch display name
        let sidebar = cx.new(|cx| {
            cx.observe(&room_list_model, |_, _, cx| cx.notify())
                .detach();
            Sidebar::new(Some(client.clone()), Some(room_list_model))
        });
        self.sidebar = sidebar.clone();

        // Fetch display name and avatar asynchronously
        let client_for_info = client.clone();
        let this = cx.entity().downgrade();
        let async_cx = cx.to_async();
        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                let display_name = client_for_info.display_name().await;
                let avatar_url = client_for_info.avatar_url().await;

                if display_name.is_some() || avatar_url.is_some() {
                    let _ = this.update(&mut async_cx.clone(), |this, cx| {
                        this.sidebar.update(cx, |sidebar, cx| {
                            if let Some(name) = display_name {
                                sidebar.display_name = name;
                            }
                            if let Some(url) = avatar_url {
                                sidebar.sync_status = "Syncing".to_string(); // Just a hint
                                sidebar.avatar_url = Some(url);
                            }
                            cx.notify();
                        });
                    });
                }
            })
            .detach();

        // Subscribe to sidebar events
        cx.subscribe(
            &sidebar,
            |this: &mut AppView, _, event: &SidebarEvent, cx| match event {
                SidebarEvent::OpenSettings => this.open_settings(cx),
            },
        )
        .detach();

        // Subscribe to sync state changes
        let async_cx = cx.to_async();
        let this_handle = cx.entity().downgrade();
        let client_clone = client.clone();

        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                if let Some(sync_service) = client_clone.sync_service().await {
                    let mut state_stream = sync_service.state();
                    while let Some(state) = state_stream.next().await {
                        let state_str = format!("{:?}", state);
                        let _ = async_cx.update(|cx| {
                            let _ = this_handle.update(cx, |this, cx| {
                                this.sync_status = state_str.clone();
                                this.sidebar.update(cx, |sidebar, cx| {
                                    sidebar.sync_status = state_str;
                                    cx.notify();
                                });
                            });
                        });
                    }
                }
            })
            .detach();

        // Check verification state and prompt if unverified
        {
            let client_for_verify = client.clone();
            let this_verify = cx.entity().downgrade();
            let async_cx = cx.to_async();
            async_cx
                .clone()
                .spawn(move |_: &mut AsyncApp| async move {
                    let sdk_client = client_for_verify.client();
                    let verification_state = sdk_client.encryption().verification_state().get();
                    tracing::info!("Post-login verification state: {:?}", verification_state);

                    let is_verified =
                        verification_state != matrix_sdk::encryption::VerificationState::Unverified;
                    let _ = async_cx.update(|cx| {
                        let _ = this_verify.update(cx, |view, cx| {
                            view.set_session_verified(is_verified, cx);
                        });
                    });

                    if verification_state == matrix_sdk::encryption::VerificationState::Unverified {
                        tracing::info!("Session is unverified, initiating self-verification");
                        // Get our own user identity and request verification
                        if let Some(user_id) = sdk_client.user_id() {
                            match sdk_client.encryption().get_user_identity(user_id).await {
                                Ok(Some(identity)) => match identity.request_verification().await {
                                    Ok(request) => {
                                        let _ = async_cx.update(|cx| {
                                            let _ = this_verify.update(cx, |view, cx| {
                                                let model = cx.new(|cx| {
                                                    let mut m = VerificationModel::new();
                                                    m.set_request(request, cx);
                                                    m
                                                });
                                                view.set_verification_model(model, cx);
                                            });
                                        });
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to request self-verification: {:?}",
                                            e
                                        );
                                    }
                                },
                                Ok(None) => {
                                    tracing::warn!("No user identity found for self-verification");
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to get user identity: {:?}", e);
                                }
                            }
                        }
                    }
                })
                .detach();
        }

        // Register handler for incoming verification requests (from other devices)
        // Uses a channel to bridge from the Send-required event handler to the GPUI context
        {
            let sdk_client = client.client();
            let this_handler = cx.entity().downgrade();
            let async_cx = cx.to_async();
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();

            // Event handler sends (sender_id, flow_id) through channel (Send-safe)
            sdk_client.add_event_handler(move |ev: matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent, _client: matrix_sdk::Client| {
                let tx = tx.clone();
                async move {
                    tracing::info!("Received incoming verification request from {} (flow: {})", ev.sender, ev.content.transaction_id);
                    let _ = tx.send((ev.sender.to_string(), ev.content.transaction_id.to_string()));
                }
            });

            // GPUI-spawned listener receives from channel and updates the model
            let client_for_handler = client.clone();
            async_cx
                .clone()
                .spawn(move |_: &mut AsyncApp| async move {
                    while let Some((sender_id, flow_id)) = rx.recv().await {
                        let sdk_client = client_for_handler.client();
                        if let Ok(sender) =
                            <&matrix_sdk::ruma::UserId>::try_from(sender_id.as_str())
                        {
                            if let Some(request) = sdk_client
                                .encryption()
                                .get_verification_request(sender, &flow_id)
                                .await
                            {
                                let _ = async_cx.update(|cx| {
                                    let _ = this_handler.update(cx, |view, cx| {
                                        if view.verification_model.is_none() {
                                            let model = cx.new(|cx| {
                                                let mut m = VerificationModel::new();
                                                m.set_request(request, cx);
                                                m
                                            });
                                            view.set_verification_model(model, cx);
                                        }
                                    });
                                });
                            }
                        }
                    }
                })
                .detach();
        }

        cx.notify();
    }

    /// Set the verification model and subscribe to its events.
    fn set_verification_model(&mut self, model: Entity<VerificationModel>, cx: &mut Context<Self>) {
        // Subscribe to verification events to clear the overlay when done
        cx.subscribe(
            &model,
            |this: &mut AppView, _, event: &VerificationEvent, cx| match event {
                VerificationEvent::Finished => {
                    tracing::info!("Verification finished, clearing overlay");
                    this.verification_model = None;
                    this.set_session_verified(true, cx);
                    cx.notify();
                }
            },
        )
        .detach();

        self.verification_model = Some(model);
        self.is_settings_open = false; // Close settings if open
        cx.notify();
    }

    /// Initiate self-verification from Settings.
    fn start_self_verification(&mut self, cx: &mut Context<Self>) {
        if let Some(client) = &self.client {
            let sdk_client = client.client();
            let this = cx.entity().downgrade();
            let async_cx = cx.to_async();

            async_cx
                .clone()
                .spawn(move |_: &mut AsyncApp| async move {
                    if let Some(user_id) = sdk_client.user_id() {
                        match sdk_client.encryption().get_user_identity(user_id).await {
                            Ok(Some(identity)) => match identity.request_verification().await {
                                Ok(request) => {
                                    let _ = async_cx.update(|cx| {
                                        let _ = this.update(cx, |view, cx| {
                                            let model = cx.new(|cx| {
                                                let mut m = VerificationModel::new();
                                                m.set_request(request, cx);
                                                m
                                            });
                                            view.set_verification_model(model, cx);
                                        });
                                    });
                                }
                                Err(e) => {
                                    tracing::error!("Failed to request verification: {:?}", e);
                                }
                            },
                            Ok(None) => {
                                tracing::warn!("No user identity found");
                            }
                            Err(e) => {
                                tracing::error!("Failed to get user identity: {:?}", e);
                            }
                        }
                    }
                })
                .detach();
        }
    }

    /// Recover the session using a recovery key.
    fn recover_with_key(&mut self, key: String, cx: &mut Context<Self>) {
        if let Some(client) = &self.client {
            let client = client.clone();
            let async_cx = cx.to_async();
            let this = cx.entity().downgrade();

            self.is_recovering = true;
            self.recovery_status = None;
            cx.notify();

            async_cx
                .clone()
                .spawn(move |_: &mut AsyncApp| async move {
                    tracing::info!("Attempting session recovery with key");
                    let recovery = client.client().encryption().recovery();
                    let result = recovery.recover(&key).await;

                    let _ = async_cx.update(|cx| {
                        let _ = this.update(cx, |view, cx| {
                            view.is_recovering = false;
                            let status = match result {
                                Ok(_) => {
                                    tracing::info!("Session recovery successful");
                                    Some(Ok(()))
                                }
                                Err(e) => {
                                    tracing::error!("Session recovery failed: {:?}", e);
                                    Some(Err(e.to_string()))
                                }
                            };
                            view.recovery_status = status.clone();
                            if status.as_ref().is_some_and(|r| r.is_ok()) {
                                view.set_session_verified(true, cx);
                            }
                            view.settings_view.update(cx, |settings, cx| {
                                settings.set_recovery_status(status, cx);
                            });
                            cx.notify();
                        });
                    });
                })
                .detach();
        }
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        if let Some(room_model) = &self.room_list_model {
            let show_rooms = room_model.read(cx).show_rooms_in_home;
            self.settings_view.update(cx, |settings, cx| {
                settings.set_show_rooms_in_home(show_rooms, cx);
                settings.set_session_verified(self.session_verified, cx);
            });
        }

        if let Some(client) = self.client.clone() {
            let async_cx = cx.to_async();
            let this = cx.entity().downgrade();
            async_cx
                .clone()
                .spawn(move |_: &mut AsyncApp| async move {
                    let verification_state =
                        client.client().encryption().verification_state().get();
                    let is_verified =
                        verification_state != matrix_sdk::encryption::VerificationState::Unverified;

                    let _ = async_cx.update(|cx| {
                        let _ = this.update(cx, |view, cx| {
                            view.set_session_verified(is_verified, cx);
                        });
                    });
                })
                .detach();
        }

        self.is_settings_open = true;
        cx.notify();
    }

    fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.is_settings_open = false;
        cx.notify();
    }

    fn logout(&mut self, cx: &mut Context<Self>) {
        tracing::info!("Logging out user");

        if let Some(client) = self.client.take() {
            tokio::spawn(async move {
                if let Err(e) = client.logout().await {
                    tracing::error!("Logout failed: {:?}", e);
                }
            });
        }

        self.is_logged_in = false;
        self.room_list_model = None;
        self.active_timeline_model = None;
        self.active_chat_view = None;
        self.verification_model = None;
        self.active_room_id = None;
        self.client = None;
        self.is_settings_open = false;
        self.session_verified = false;
        self.sidebar = cx.new(|_| Sidebar::new(None, None));
        cx.notify();
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Initialize ChatView if needed
        if self.active_timeline_model.is_some() && self.active_chat_view.is_none() {
            let model = self.active_timeline_model.as_ref().unwrap().clone();
            let room_list_model = self.room_list_model.as_ref().unwrap().clone();
            let chat_view = cx.new(|cx| ChatView::new(room_list_model, model, window, cx));
            self.active_chat_view = Some(chat_view);
        }
        let theme = cx.onedark_theme();

        if !self.is_logged_in {
            return div().size_full().child(self.login_view.clone());
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
                        // Verification Overlay
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
                        // Empty State
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
                                            crate::components::remote_image::RemoteImage::new(
                                                url.clone(),
                                            )
                                            .size(px(96.0))
                                            .avatar()
                                            .into_any_element()
                                        } else {
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
            // Settings overlay
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
