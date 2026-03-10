use super::AppView;
use crate::models::ui_preferences;
use gpui::AsyncApp;
use gpui::*;
use rivet_core::client::RivetClient;

impl AppView {
    pub(crate) fn open_settings(&mut self, cx: &mut Context<Self>) {
        let preferences = ui_preferences::ui_preferences(cx);
        if let Some(room_model) = &self.room_list_model {
            let room_model = room_model.read(cx);
            let show_rooms_in_home = room_model.show_rooms_in_home;
            let show_other_rooms = room_model.show_other_rooms;
            let remember_last_room = room_model.remember_last_room;
            self.settings_view.update(cx, |settings, cx| {
                settings.sync_navigation_preferences(
                    show_rooms_in_home,
                    preferences.show_sidecart,
                    show_other_rooms,
                    remember_last_room,
                    cx,
                );
                settings.set_session_verified(self.session_verified, cx);
            });
        } else {
            self.settings_view.update(cx, |settings, cx| {
                settings.sync_navigation_preferences(
                    preferences.show_rooms_in_home,
                    preferences.show_sidecart,
                    preferences.show_other_rooms,
                    preferences.remember_last_room,
                    cx,
                );
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

    pub(crate) fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.is_settings_open = false;
        cx.notify();
    }

    pub(crate) fn delete_all_local_data(&mut self, cx: &mut Context<Self>) {
        tracing::warn!("Deleting all local Rivet data stores");

        let client = self.client.take();
        self.reset_logged_out_state(cx);
        cx.notify();

        let async_cx = cx.to_async();
        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                if let Some(client) = client {
                    if let Err(error) = client.shutdown().await {
                        tracing::error!(
                            "Failed to stop active profile before data deletion: {:?}",
                            error
                        );
                    }
                    drop(client);
                }

                if let Err(error) = RivetClient::delete_all_local_data() {
                    tracing::error!("Failed to delete local Rivet data: {:?}", error);
                }
            })
            .detach();
    }
}
