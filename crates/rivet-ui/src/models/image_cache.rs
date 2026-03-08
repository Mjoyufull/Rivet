use gpui::*;
use image::ImageFormat;
use image::imageops::FilterType;
use matrix_sdk::media::{MediaFormat, MediaRequestParameters, MediaThumbnailSettings};
use matrix_sdk::reqwest;
use matrix_sdk::ruma::events::room::MediaSource;
use rivet_core::client::RivetClient;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct ImageCache {
    client: Option<RivetClient>,
    cache: HashMap<String, Arc<Image>>,
    pending: HashSet<String>,
    failed_until: HashMap<String, Instant>,
    // Track which URLs are avatars (for thumbnail optimization)
    avatar_urls: HashSet<String>,
}

impl Global for ImageCache {}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            client: None,
            cache: HashMap::new(),
            pending: HashSet::new(),
            failed_until: HashMap::new(),
            avatar_urls: HashSet::new(),
        }
    }

    pub fn init(client: RivetClient, cx: &mut App) {
        cx.update_global::<Self, _>(|this, _cx| {
            this.client = Some(client);
        });
    }

    /// Mark a URL as an avatar for thumbnail optimization
    pub fn mark_as_avatar(&mut self, url: String) {
        self.avatar_urls.insert(url);
    }

    pub fn get(&mut self, url: String, cx: &mut App) -> Option<Arc<Image>> {
        if let Some(image) = self.cache.get(&url) {
            return Some(image.clone());
        }
        if let Some(until) = self.failed_until.get(&url) {
            if Instant::now() < *until {
                return None;
            }
            self.failed_until.remove(&url);
        }

        if !self.pending.contains(&url) {
            self.pending.insert(url.clone());
            let url_clone = url.clone();
            let async_cx = cx.to_async();
            let client = self.client.clone();
            let is_avatar = self.avatar_urls.contains(&url);

            // Spawn background task to fetch image
            async_cx
                .clone()
                .spawn(move |_: &mut AsyncApp| async move {
                    let mxc_uri = if url_clone.starts_with("mxc://") {
                        Some(url_clone.clone())
                    } else if let Some(pos) = url_clone.find("/_matrix/media/v3/download/") {
                        let mxc_part = &url_clone[pos + "/_matrix/media/v3/download/".len()..];
                        Some(format!("mxc://{}", mxc_part))
                    } else if let Some(pos) = url_clone.find("/_matrix/media/v3/thumbnail/") {
                        let mxc_part = &url_clone[pos + "/_matrix/media/v3/thumbnail/".len()..];
                        let clean_mxc = mxc_part.split('?').next().unwrap_or(mxc_part);
                        Some(format!("mxc://{}", clean_mxc))
                    } else {
                        None
                    };

                    let result = if let (Some(client), Some(mxc_str)) = (&client, mxc_uri) {
                        let source = MediaSource::Plain(matrix_sdk::ruma::OwnedMxcUri::from(
                            mxc_str.clone(),
                        ));
                        Self::fetch_from_matrix_media(client, source, is_avatar, mxc_str).await
                    } else {
                        Self::fetch_with_reqwest(&url_clone).await
                    };

                    Self::process_image_result(result, url_clone, is_avatar, None, async_cx).await;
                })
                .detach();
        }

        None
    }

    pub fn get_media_source(
        &mut self,
        source: MediaSource,
        url_key: String,
        mimetype: Option<String>,
        cx: &mut App,
    ) -> Option<Arc<Image>> {
        if let Some(image) = self.cache.get(&url_key) {
            return Some(image.clone());
        }
        if let Some(until) = self.failed_until.get(&url_key) {
            if Instant::now() < *until {
                return None;
            }
            self.failed_until.remove(&url_key);
        }

        if !self.pending.contains(&url_key) {
            self.pending.insert(url_key.clone());
            let url_clone = url_key.clone();
            let async_cx = cx.to_async();
            let client = self.client.clone();

            // Spawn background task to fetch image
            async_cx
                .clone()
                .spawn(move |_: &mut AsyncApp| async move {
                    let result = if let Some(client) = &client {
                        let mxc_str = match &source {
                            MediaSource::Plain(mxc) => mxc.to_string(),
                            MediaSource::Encrypted(enc) => enc.url.to_string(),
                        };
                        Self::fetch_from_matrix_media(client, source, false, mxc_str).await
                    } else {
                        Err(anyhow::anyhow!("Client not available"))
                    };

                    Self::process_image_result(result, url_clone, false, mimetype, async_cx).await;
                })
                .detach();
        }

        None
    }

    async fn process_image_result(
        result: anyhow::Result<(Vec<u8>, Option<String>)>,
        url_clone: String,
        is_avatar: bool,
        fallback_mimetype: Option<String>,
        async_cx: AsyncApp,
    ) {
        match result {
            Ok((bytes, content_type)) => {
                let mut bytes = bytes.to_vec();
                let mut content_type = content_type.or(fallback_mimetype);

                if is_avatar && let Some(processed) = Self::prepare_avatar_bytes(&bytes) {
                    bytes = processed;
                    content_type = Some("image/png".to_string());
                }

                // Detect format
                let format = if let Some(ref ct) = content_type {
                    match ct.as_str() {
                        "image/png" => Some(gpui::ImageFormat::Png),
                        "image/jpeg" | "image/jpg" => Some(gpui::ImageFormat::Jpeg),
                        "image/gif" => Some(gpui::ImageFormat::Gif),
                        "image/webp" => Some(gpui::ImageFormat::Webp),
                        _ => None,
                    }
                } else {
                    None
                };

                // Fallback to magic numbers if header is missing or unknown
                let format = format.or_else(|| {
                    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                        Some(gpui::ImageFormat::Png)
                    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
                        Some(gpui::ImageFormat::Jpeg)
                    } else if bytes.starts_with(&[0x47, 0x49, 0x46, 0x38]) {
                        Some(gpui::ImageFormat::Gif)
                    } else if bytes.starts_with(&[0x52, 0x49, 0x46, 0x46])
                        && bytes.get(8..12) == Some(b"WEBP")
                    {
                        Some(gpui::ImageFormat::Webp)
                    } else if let Ok(s) = std::str::from_utf8(&bytes[..bytes.len().min(100)]) {
                        if s.contains("<svg") {
                            Some(gpui::ImageFormat::Svg)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

                if let Some(format) = format {
                    let image = Arc::new(Image::from_bytes(format, bytes));
                    let _ = async_cx.update(|cx: &mut App| {
                        cx.update_global::<Self, _>(|this, _cx| {
                            this.cache.insert(url_clone.clone(), image);
                            this.failed_until.remove(&url_clone);
                            this.pending.remove(&url_clone);
                        });
                    });
                } else if let Some(png_bytes) = Self::decode_to_png_bytes(&bytes) {
                    let image = Arc::new(Image::from_bytes(gpui::ImageFormat::Png, png_bytes));
                    let _ = async_cx.update(|cx: &mut App| {
                        cx.update_global::<Self, _>(|this, _cx| {
                            this.cache.insert(url_clone.clone(), image);
                            this.failed_until.remove(&url_clone);
                            this.pending.remove(&url_clone);
                        });
                    });
                } else {
                    tracing::error!(
                        "Unknown image format for {}. Content-Type: {:?}",
                        url_clone,
                        content_type
                    );
                    let _ = async_cx.update(|cx: &mut App| {
                        cx.update_global::<Self, _>(|this, _cx| {
                            this.failed_until.insert(
                                url_clone.clone(),
                                Instant::now() + Duration::from_secs(30),
                            );
                            this.pending.remove(&url_clone);
                        });
                    });
                }
            }
            Err(e) => {
                tracing::error!("Failed to fetch image {}: {:?}", url_clone, e);
                let _ = async_cx.update(|cx: &mut App| {
                    cx.update_global::<Self, _>(|this, _cx| {
                        this.failed_until
                            .insert(url_clone.clone(), Instant::now() + Duration::from_secs(10));
                        this.pending.remove(&url_clone);
                    });
                });
            }
        }
    }

    async fn fetch_from_matrix_media(
        client: &RivetClient,
        source: MediaSource,
        is_avatar: bool,
        mxc_str: String,
    ) -> anyhow::Result<(Vec<u8>, Option<String>)> {
        // mxc_str comes from source
        let _mxc = matrix_sdk::ruma::OwnedMxcUri::from(mxc_str.to_string());

        if is_avatar {
            let thumb = MediaRequestParameters {
                source: source.clone(),
                format: MediaFormat::Thumbnail(MediaThumbnailSettings::with_method(
                    matrix_sdk::ruma::api::client::media::get_content_thumbnail::v3::Method::Crop,
                    matrix_sdk::ruma::uint!(64),
                    matrix_sdk::ruma::uint!(64),
                )),
            };

            match client
                .client()
                .media()
                .get_media_content(&thumb, true)
                .await
            {
                Ok(bytes) => return Ok((bytes.to_vec(), None)),
                Err(err) => {
                    tracing::debug!(
                        "Thumbnail fetch failed for {mxc_str}, falling back to file media: {err:?}"
                    );
                }
            }
        }

        let file = MediaRequestParameters {
            source,
            format: MediaFormat::File,
        };

        client
            .client()
            .media()
            .get_media_content(&file, true)
            .await
            .map(|bytes| (bytes.to_vec(), None))
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn fetch_with_reqwest(url: &str) -> anyhow::Result<(Vec<u8>, Option<String>)> {
        let resp = reqwest::get(url).await?;
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let bytes = resp.bytes().await?.to_vec();
        Ok((bytes, content_type))
    }

    fn prepare_avatar_bytes(input: &[u8]) -> Option<Vec<u8>> {
        let image = image::load_from_memory(input).ok()?;
        let mut rgba = image.to_rgba8();
        let (mut width, mut height) = rgba.dimensions();

        if width == 0 || height == 0 {
            return None;
        }

        // Trim transparent padding first to avoid "half avatar" artifacts from padded icons.
        let mut min_x = width;
        let mut min_y = height;
        let mut max_x = 0;
        let mut max_y = 0;
        let mut has_opaque = false;

        for y in 0..height {
            for x in 0..width {
                let alpha = rgba.get_pixel(x, y)[3];
                if alpha > 10 {
                    has_opaque = true;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }

        if has_opaque {
            let trim_w = (max_x - min_x) + 1;
            let trim_h = (max_y - min_y) + 1;
            rgba = image::imageops::crop_imm(&rgba, min_x, min_y, trim_w, trim_h).to_image();
            width = trim_w;
            height = trim_h;
        }

        let side = width.min(height);
        let offset_x = (width - side) / 2;
        let offset_y = (height - side) / 2;

        let cropped = image::imageops::crop_imm(&rgba, offset_x, offset_y, side, side).to_image();
        let resized = image::imageops::resize(&cropped, 96, 96, FilterType::Lanczos3);
        let output = image::DynamicImage::ImageRgba8(resized);

        let mut encoded = Vec::new();
        let mut cursor = Cursor::new(&mut encoded);
        output.write_to(&mut cursor, ImageFormat::Png).ok()?;
        Some(encoded)
    }

    fn decode_to_png_bytes(input: &[u8]) -> Option<Vec<u8>> {
        let image = image::load_from_memory(input).ok()?;
        let output = image.to_rgba8();
        let mut encoded = Vec::new();
        let mut cursor = Cursor::new(&mut encoded);
        image::DynamicImage::ImageRgba8(output)
            .write_to(&mut cursor, ImageFormat::Png)
            .ok()?;
        Some(encoded)
    }
}
