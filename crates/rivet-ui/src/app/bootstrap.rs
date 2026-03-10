use super::state::AppView;
use crate::auth::{LoginEvent, LoginView};
use crate::models::image_cache::ImageCache;
use crate::models::ui_preferences;
use crate::settings::{SettingsEvent, SettingsView};
use crate::sidebar::Sidebar;
use gpui::AsyncApp;
use gpui::*;
use gpui_component::Root;
use rivet_core::client::RivetClient;

pub fn build_root(window: &mut Window, cx: &mut App) -> Entity<Root> {
    let sidebar = cx.new(|_| Sidebar::new(None, None));
    let login_view = cx.new(|cx| LoginView::new(window, cx));
    let settings_view = cx.new(|cx| SettingsView::new(window, cx));

    let main_view = cx.new(|cx| {
        let view = AppView::new(sidebar, login_view.clone(), settings_view.clone());

        cx.subscribe(
            &login_view,
            |this: &mut AppView, _, event: &LoginEvent, cx| match event {
                LoginEvent::Success(client) => {
                    this.login(client.clone(), cx);
                }
            },
        )
        .detach();

        cx.subscribe(
            &settings_view,
            |this: &mut AppView, _, event: &SettingsEvent, cx| match event {
                SettingsEvent::Close => this.close_settings(cx),
                SettingsEvent::Logout => this.logout(cx),
                SettingsEvent::DeleteAllData => this.delete_all_local_data(cx),
                SettingsEvent::VerifySession => this.start_self_verification(cx),
                SettingsEvent::RecoverWithKey(key) => this.recover_with_key(key.clone(), cx),
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
                SettingsEvent::SetShowSidecart(val) => {
                    ui_preferences::set_show_sidecart(*val, cx);
                }
                SettingsEvent::SetShowOtherRooms(val) => {
                    if let Some(room_model) = &this.room_list_model {
                        room_model.update(cx, |model, cx| {
                            model.set_show_other_rooms(*val, cx);
                        });
                    } else {
                        ui_preferences::set_show_other_rooms(*val, cx);
                    }
                }
                SettingsEvent::SetRememberLastRoom(val) => {
                    if let Some(room_model) = &this.room_list_model {
                        room_model.update(cx, |model, cx| {
                            model.set_remember_last_room(*val, cx);
                        });
                    } else {
                        ui_preferences::set_remember_last_room(*val, cx);
                    }
                }
            },
        )
        .detach();

        view
    });

    let this = main_view.downgrade();
    let async_cx = cx.to_async();
    async_cx
        .clone()
        .spawn(move |_: &mut AsyncApp| {
            let this = this.clone();
            async move {
                if let Ok(Some(client)) = RivetClient::restore().await {
                    let _ = async_cx.update(|cx| {
                        let _ = this.update(cx, |view, cx| {
                            ImageCache::init(client.clone(), cx);
                            view.login(client, cx);
                        });
                    });
                }
            }
        })
        .detach();

    cx.new(|cx| Root::new(main_view, window, cx))
}
