use super::AppView;
use crate::security::verification::{VerificationEvent, VerificationModel};
use gpui::AsyncApp;
use gpui::*;

impl AppView {
    pub(crate) fn set_verification_model(
        &mut self,
        model: Entity<VerificationModel>,
        cx: &mut Context<Self>,
    ) {
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
        self.is_settings_open = false;
        cx.notify();
    }

    pub(crate) fn start_self_verification(&mut self, cx: &mut Context<Self>) {
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

    pub(crate) fn recover_with_key(&mut self, key: String, cx: &mut Context<Self>) {
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
}
