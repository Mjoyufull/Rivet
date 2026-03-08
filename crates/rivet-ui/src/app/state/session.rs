use super::AppView;
use crate::models::image_cache::ImageCache;
use crate::rooms::RoomListModel;
use crate::security::verification::VerificationModel;
use crate::sidebar::{Sidebar, SidebarEvent};
use crate::timeline::TimelineModel;
use gpui::AsyncApp;
use gpui::*;
use matrix_sdk::ruma::RoomId;
use matrix_sdk_ui::timeline::RoomExt;
use rivet_core::client::RivetClient;
use std::sync::Arc;

impl AppView {
    pub(crate) fn login(&mut self, client: RivetClient, cx: &mut Context<Self>) {
        tracing::info!("Initializing UI for logged in user");
        self.client = Some(client.clone());
        ImageCache::init(client.clone(), cx);

        let room_list_model = RoomListModel::new(cx);
        RoomListModel::init(room_list_model.clone(), client.clone(), cx);
        self.room_list_model = Some(room_list_model.clone());
        self.observe_room_selection(&room_list_model, cx);

        self.is_logged_in = true;
        self.install_sidebar(client.clone(), room_list_model, cx);
        self.spawn_sidebar_profile_refresh(client.clone(), cx);
        self.spawn_sync_status_listener(client.clone(), cx);
        self.spawn_post_login_self_verification(client.clone(), cx);
        self.install_verification_request_handler(client, cx);
        cx.notify();
    }

    fn observe_room_selection(
        &self,
        room_list_model: &Entity<RoomListModel>,
        cx: &mut Context<Self>,
    ) {
        cx.observe(room_list_model, |this: &mut AppView, model, cx| {
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
                            async_cx
                                .clone()
                                .spawn(move |_: &mut AsyncApp| async move {
                                    tracing::info!("Building timeline for room: {}", room_id);
                                    match room.timeline_builder().build().await {
                                        Ok(timeline) => {
                                            let timeline = Arc::new(timeline);
                                            let _ = async_cx.update(|cx| {
                                                let _ = this_handle.update(
                                                    cx,
                                                    |view, cx: &mut Context<AppView>| {
                                                        tracing::info!(
                                                            "Initializing timeline model for room: {}",
                                                            room_id
                                                        );
                                                        let homeserver_url =
                                                            client.homeserver_url().to_string();
                                                        let model = TimelineModel::new(
                                                            room,
                                                            timeline,
                                                            homeserver_url,
                                                            cx,
                                                        );
                                                        TimelineModel::init(model.clone(), cx);
                                                        view.active_timeline_model = Some(model);
                                                        cx.notify();
                                                    },
                                                );
                                            });
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                "Failed to build timeline for room {}: {:?}",
                                                room_id,
                                                e
                                            );
                                        }
                                    }
                                })
                                .detach();
                        } else {
                            tracing::warn!("Room not found in client: {}", room_id_str);
                        }
                    } else {
                        tracing::error!("Failed to parse room ID: {}", room_id_str);
                    }
                }
                cx.notify();
            }
        })
        .detach();
    }

    fn install_sidebar(
        &mut self,
        client: RivetClient,
        room_list_model: Entity<RoomListModel>,
        cx: &mut Context<Self>,
    ) {
        let sidebar = cx.new(|cx| {
            cx.observe(&room_list_model, |_, _, cx| cx.notify())
                .detach();
            Sidebar::new(Some(client.clone()), Some(room_list_model))
        });
        self.sidebar = sidebar.clone();

        cx.subscribe(
            &sidebar,
            |this: &mut AppView, _, event: &SidebarEvent, cx| match event {
                SidebarEvent::OpenSettings => this.open_settings(cx),
            },
        )
        .detach();
    }

    fn spawn_sidebar_profile_refresh(&self, client: RivetClient, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let async_cx = cx.to_async();
        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                let display_name = client.display_name().await;
                let avatar_url = client.avatar_url().await;

                if display_name.is_some() || avatar_url.is_some() {
                    let _ = this.update(&mut async_cx.clone(), |this, cx| {
                        this.sidebar.update(cx, |sidebar, cx| {
                            if let Some(name) = display_name {
                                sidebar.display_name = name;
                            }
                            if let Some(url) = avatar_url {
                                sidebar.sync_status = "Syncing".to_string();
                                sidebar.avatar_url = Some(url);
                            }
                            cx.notify();
                        });
                    });
                }
            })
            .detach();
    }

    fn spawn_sync_status_listener(&self, client: RivetClient, cx: &mut Context<Self>) {
        let async_cx = cx.to_async();
        let this_handle = cx.entity().downgrade();

        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                if let Some(sync_service) = client.sync_service().await {
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
    }

    fn spawn_post_login_self_verification(&self, client: RivetClient, cx: &mut Context<Self>) {
        let this_verify = cx.entity().downgrade();
        let async_cx = cx.to_async();
        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                let sdk_client = client.client();
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
                                    tracing::warn!("Failed to request self-verification: {:?}", e);
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

    fn install_verification_request_handler(&self, client: RivetClient, cx: &mut Context<Self>) {
        let sdk_client = client.client();
        let this_handler = cx.entity().downgrade();
        let async_cx = cx.to_async();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();

        sdk_client.add_event_handler(move |ev: matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent, _client: matrix_sdk::Client| {
            let tx = tx.clone();
            async move {
                tracing::info!(
                    "Received incoming verification request from {} (flow: {})",
                    ev.sender,
                    ev.content.transaction_id
                );
                let _ = tx.send((ev.sender.to_string(), ev.content.transaction_id.to_string()));
            }
        });

        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                while let Some((sender_id, flow_id)) = rx.recv().await {
                    let sdk_client = client.client();
                    if let Ok(sender) = <&matrix_sdk::ruma::UserId>::try_from(sender_id.as_str()) {
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

    pub(crate) fn logout(&mut self, cx: &mut Context<Self>) {
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
