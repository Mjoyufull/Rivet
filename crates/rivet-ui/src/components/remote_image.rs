use crate::models::appearance::{avatar_radius_for, get_radius};
use gpui::ObjectFit;
use gpui::*;
use gpui_component::StyledExt;
use std::sync::Arc;

#[derive(IntoElement)]
pub struct RemoteImage {
    url: String,
    source: Option<matrix_sdk::ruma::events::room::MediaSource>,
    mimetype: Option<String>,
    size: Option<Pixels>,
    is_avatar: bool,
    object_fit: Option<ObjectFit>,
}

impl RemoteImage {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            source: None,
            mimetype: None,
            size: None,
            is_avatar: false,
            object_fit: None,
        }
    }

    pub fn with_source(mut self, source: matrix_sdk::ruma::events::room::MediaSource) -> Self {
        self.source = Some(source);
        self
    }

    pub fn with_mimetype(mut self, mimetype: Option<String>) -> Self {
        self.mimetype = mimetype;
        self
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = Some(size);
        self
    }

    pub fn object_fit(mut self, fit: ObjectFit) -> Self {
        self.object_fit = Some(fit);
        self
    }

    /// Mark this image as an avatar for thumbnail optimization
    pub fn avatar(mut self) -> Self {
        self.is_avatar = true;
        // Avatars usually look best with cover fit
        if self.object_fit.is_none() {
            self.object_fit = Some(ObjectFit::Cover);
        }
        self
    }
}

impl RenderOnce for RemoteImage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Mark as avatar for thumbnail optimization
        if self.is_avatar {
            cx.update_global::<crate::models::image_cache::ImageCache, _>(|this, _cx| {
                this.mark_as_avatar(self.url.clone());
            });
        }

        let image = if let Some(source) = self.source {
            cx.update_global::<crate::models::image_cache::ImageCache, Option<Arc<Image>>>(
                |this, cx| {
                    this.get_media_source(source, self.url.clone(), self.mimetype.clone(), cx)
                },
            )
        } else {
            cx.update_global::<crate::models::image_cache::ImageCache, Option<Arc<Image>>>(
                |this, cx| this.get(self.url.clone(), cx),
            )
        };

        let radius = if let Some(size) = self.size {
            avatar_radius_for(size, cx)
        } else {
            get_radius(cx)
        };
        let corner_radii = Corners::all(radius);

        if let Some(image) = image {
            let element = img(ImageSource::Image(image.clone()))
                .object_fit(self.object_fit.unwrap_or(ObjectFit::Cover))
                .corner_radii(corner_radii);

            if let Some(size) = self.size {
                div()
                    .size(size)
                    .flex_shrink_0()
                    .overflow_hidden()
                    .corner_radii(corner_radii)
                    .child(element.size_full())
                    .into_any_element()
            } else {
                element.into_any_element()
            }
        } else {
            // Loading placeholder
            let placeholder = div()
                .bg(rgb(0x282c34))
                .flex()
                .items_center()
                .justify_center()
                .child(div().text_xs().text_color(rgb(0x5c6370)).child("..."));

            if let Some(size) = self.size {
                placeholder
                    .size(size)
                    .flex_shrink_0()
                    .corner_radii(corner_radii)
                    .into_any_element()
            } else {
                placeholder.into_any_element()
            }
        }
    }
}
