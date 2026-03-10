use gpui::*;
use rivet_ui::models::image_cache::ImageCache;
use rivet_ui::{Assets, app, models, theme};

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
        models::ui_preferences::init(cx);
        cx.set_global(ImageCache::new());
        gpui_component::Theme::global_mut(cx).colors =
            theme::onedark::ONEDARK_THEME.to_theme_color();

        cx.open_window(WindowOptions::default(), app::build_root)
            .unwrap();
    });
}
