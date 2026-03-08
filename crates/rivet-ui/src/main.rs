mod app;
mod auth;
mod components;
mod models;
mod rooms;
mod security;
mod settings;
mod sidebar;
mod theme;
mod timeline;

use gpui::*;

use crate::models::image_cache::ImageCache;

use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "../../assets/"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(|f| Cow::Owned(f.data.into_owned())))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
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

    Application::new().with_assets(Assets).run(|cx| {
        gpui_component::init(cx);
        models::appearance::init(cx);
        cx.set_global(ImageCache::new());
        gpui_component::Theme::global_mut(cx).colors =
            theme::onedark::ONEDARK_THEME.to_theme_color();

        cx.open_window(WindowOptions::default(), app::build_root)
            .unwrap();
    });
}
