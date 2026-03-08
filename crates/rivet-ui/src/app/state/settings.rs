use super::AppView;
use gpui::AsyncApp;
use gpui::*;

impl AppView {
    pub(crate) fn open_settings(&mut self, cx: &mut Context<Self>) {
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

    pub(crate) fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.is_settings_open = false;
        cx.notify();
    }
}
