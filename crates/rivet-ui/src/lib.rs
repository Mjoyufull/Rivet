pub mod app;
pub mod auth;
pub mod components;
pub mod models;
pub mod rooms;
pub mod security;
pub mod settings;
pub mod sidebar;
pub mod theme;
pub mod timeline;

use gpui::{AssetSource, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "../../assets/"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(|f| Cow::Owned(f.data.into_owned())))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| {
                if p.starts_with(path) {
                    Some(p.into())
                } else {
                    None
                }
            })
            .collect())
    }
}
