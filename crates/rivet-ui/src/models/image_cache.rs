use crate::perf;
use gpui::*;
use image::ImageFormat;
use image::imageops::FilterType;
use matrix_sdk::media::{MediaFormat, MediaRequestParameters, MediaThumbnailSettings};
use matrix_sdk::reqwest;
use matrix_sdk::ruma::events::room::MediaSource;
use rivet_core::client::RivetClient;
use rivet_core::store;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_CONCURRENT_AVATAR_REQUESTS: usize = 6;
const MAX_CONCURRENT_MEDIA_REQUESTS: usize = 2;
const MAX_LOW_PRIORITY_AVATAR_REQUESTS: usize = 2;
const MAX_LOW_PRIORITY_MEDIA_REQUESTS: usize = 1;
const IMAGE_NOTIFY_BATCH_MS: u64 = 16;

#[derive(Clone, Copy)]
enum ImageBucket {
    Avatar,
    Media,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImagePriority {
    High,
    Normal,
    Low,
}

#[derive(Clone)]
enum ImageRequestKind {
    Url {
        url: String,
        is_avatar: bool,
    },
    MediaSource {
        source: MediaSource,
        mimetype: Option<String>,
    },
}

#[derive(Clone)]
struct QueuedImageRequest {
    bucket: ImageBucket,
    priority: ImagePriority,
    key: String,
    request: ImageRequestKind,
    client: Option<RivetClient>,
}

#[derive(Default)]
struct BucketQueues {
    high: VecDeque<QueuedImageRequest>,
    normal: VecDeque<QueuedImageRequest>,
    low: VecDeque<QueuedImageRequest>,
}

impl BucketQueues {
    fn push(&mut self, request: QueuedImageRequest) {
        match request.priority {
            ImagePriority::High => self.high.push_back(request),
            ImagePriority::Normal => self.normal.push_back(request),
            ImagePriority::Low => self.low.push_back(request),
        }
    }

    fn pop_next(&mut self, low_in_flight: usize, low_limit: usize) -> Option<QueuedImageRequest> {
        self.high
            .pop_front()
            .or_else(|| self.normal.pop_front())
            .or_else(|| {
                if low_in_flight < low_limit {
                    self.low.pop_front()
                } else {
                    None
                }
            })
    }
}

pub struct ImageCache {
    client: Option<RivetClient>,
    avatar_cache: HashMap<String, Arc<Image>>,
    avatar_pending: HashSet<String>,
    avatar_failed_until: HashMap<String, Instant>,
    avatar_waiters: HashMap<String, HashSet<EntityId>>,
    avatar_queue: BucketQueues,
    avatar_in_flight: usize,
    avatar_low_in_flight: usize,
    media_cache: HashMap<String, Arc<Image>>,
    media_pending: HashSet<String>,
    media_failed_until: HashMap<String, Instant>,
    media_waiters: HashMap<String, HashSet<EntityId>>,
    media_queue: BucketQueues,
    media_in_flight: usize,
    media_low_in_flight: usize,
    notify_scheduled: bool,
    pending_notify_views: HashSet<EntityId>,
}

impl Global for ImageCache {}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            client: None,
            avatar_cache: HashMap::new(),
            avatar_pending: HashSet::new(),
            avatar_failed_until: HashMap::new(),
            avatar_waiters: HashMap::new(),
            avatar_queue: BucketQueues::default(),
            avatar_in_flight: 0,
            avatar_low_in_flight: 0,
            media_cache: HashMap::new(),
            media_pending: HashSet::new(),
            media_failed_until: HashMap::new(),
            media_waiters: HashMap::new(),
            media_queue: BucketQueues::default(),
            media_in_flight: 0,
            media_low_in_flight: 0,
            notify_scheduled: false,
            pending_notify_views: HashSet::new(),
        }
    }

    pub fn init(client: RivetClient, cx: &mut App) {
        cx.update_global::<Self, _>(|this, _cx| {
            this.client = Some(client);
        });
    }

    pub fn get_avatar(
        &mut self,
        url: String,
        requester: EntityId,
        priority: ImagePriority,
        cx: &mut App,
    ) -> Option<Arc<Image>> {
        if let Some(image) = self.lookup_cached(ImageBucket::Avatar, &url) {
            return Some(image);
        }

        self.register_waiter(ImageBucket::Avatar, &url, requester);
        if !self.begin_request(ImageBucket::Avatar, &url) {
            return None;
        }

        let url_clone = url.clone();
        let async_cx = cx.to_async();
        let client = self.client.clone();
        let request = QueuedImageRequest {
            bucket: ImageBucket::Avatar,
            priority,
            key: url_clone.clone(),
            request: ImageRequestKind::Url {
                url: url_clone,
                is_avatar: true,
            },
            client,
        };
        if let Some(request) = self.enqueue_or_start(request) {
            Self::spawn_request(async_cx, request);
        }

        None
    }

    pub fn get(
        &mut self,
        url: String,
        requester: EntityId,
        priority: ImagePriority,
        cx: &mut App,
    ) -> Option<Arc<Image>> {
        if let Some(image) = self.lookup_cached(ImageBucket::Media, &url) {
            return Some(image);
        }

        self.register_waiter(ImageBucket::Media, &url, requester);
        if !self.begin_request(ImageBucket::Media, &url) {
            return None;
        }

        let url_clone = url.clone();
        let async_cx = cx.to_async();
        let client = self.client.clone();
        let request = QueuedImageRequest {
            bucket: ImageBucket::Media,
            priority,
            key: url_clone.clone(),
            request: ImageRequestKind::Url {
                url: url_clone,
                is_avatar: false,
            },
            client,
        };
        if let Some(request) = self.enqueue_or_start(request) {
            Self::spawn_request(async_cx, request);
        }

        None
    }

    pub fn get_media_source(
        &mut self,
        source: MediaSource,
        url_key: String,
        mimetype: Option<String>,
        requester: EntityId,
        priority: ImagePriority,
        cx: &mut App,
    ) -> Option<Arc<Image>> {
        if let Some(image) = self.lookup_cached(ImageBucket::Media, &url_key) {
            return Some(image);
        }

        self.register_waiter(ImageBucket::Media, &url_key, requester);
        if !self.begin_request(ImageBucket::Media, &url_key) {
            return None;
        }

        let url_clone = url_key.clone();
        let async_cx = cx.to_async();
        let client = self.client.clone();
        let request = QueuedImageRequest {
            bucket: ImageBucket::Media,
            priority,
            key: url_clone,
            request: ImageRequestKind::MediaSource { source, mimetype },
            client,
        };
        if let Some(request) = self.enqueue_or_start(request) {
            Self::spawn_request(async_cx, request);
        }

        None
    }

    fn register_waiter(&mut self, bucket: ImageBucket, key: &str, requester: EntityId) {
        let waiters = match bucket {
            ImageBucket::Avatar => &mut self.avatar_waiters,
            ImageBucket::Media => &mut self.media_waiters,
        };
        waiters
            .entry(key.to_string())
            .or_default()
            .insert(requester);
    }

    fn lookup_cached(&mut self, bucket: ImageBucket, key: &str) -> Option<Arc<Image>> {
        let cache = match bucket {
            ImageBucket::Avatar => &self.avatar_cache,
            ImageBucket::Media => &self.media_cache,
        };
        if let Some(image) = cache.get(key) {
            return Some(image.clone());
        }

        let failed_until = match bucket {
            ImageBucket::Avatar => &mut self.avatar_failed_until,
            ImageBucket::Media => &mut self.media_failed_until,
        };
        if let Some(until) = failed_until.get(key) {
            if Instant::now() < *until {
                return None;
            }
            failed_until.remove(key);
        }

        None
    }

    fn begin_request(&mut self, bucket: ImageBucket, key: &str) -> bool {
        let pending = match bucket {
            ImageBucket::Avatar => &mut self.avatar_pending,
            ImageBucket::Media => &mut self.media_pending,
        };

        if pending.contains(key) {
            return false;
        }

        pending.insert(key.to_string());
        true
    }

    fn enqueue_or_start(&mut self, request: QueuedImageRequest) -> Option<QueuedImageRequest> {
        let (in_flight, low_in_flight, limit, low_limit, queue) = match request.bucket {
            ImageBucket::Avatar => (
                &mut self.avatar_in_flight,
                &mut self.avatar_low_in_flight,
                MAX_CONCURRENT_AVATAR_REQUESTS,
                MAX_LOW_PRIORITY_AVATAR_REQUESTS,
                &mut self.avatar_queue,
            ),
            ImageBucket::Media => (
                &mut self.media_in_flight,
                &mut self.media_low_in_flight,
                MAX_CONCURRENT_MEDIA_REQUESTS,
                MAX_LOW_PRIORITY_MEDIA_REQUESTS,
                &mut self.media_queue,
            ),
        };

        let can_start = *in_flight < limit
            && (request.priority != ImagePriority::Low || *low_in_flight < low_limit);

        if can_start {
            *in_flight += 1;
            if request.priority == ImagePriority::Low {
                *low_in_flight += 1;
            }
            Some(request)
        } else {
            queue.push(request);
            None
        }
    }

    fn complete_request(
        &mut self,
        bucket: ImageBucket,
        completed_priority: ImagePriority,
    ) -> Vec<QueuedImageRequest> {
        let (in_flight, low_in_flight, queue, limit, low_limit) = match bucket {
            ImageBucket::Avatar => (
                &mut self.avatar_in_flight,
                &mut self.avatar_low_in_flight,
                &mut self.avatar_queue,
                MAX_CONCURRENT_AVATAR_REQUESTS,
                MAX_LOW_PRIORITY_AVATAR_REQUESTS,
            ),
            ImageBucket::Media => (
                &mut self.media_in_flight,
                &mut self.media_low_in_flight,
                &mut self.media_queue,
                MAX_CONCURRENT_MEDIA_REQUESTS,
                MAX_LOW_PRIORITY_MEDIA_REQUESTS,
            ),
        };

        *in_flight = in_flight.saturating_sub(1);
        if completed_priority == ImagePriority::Low {
            *low_in_flight = low_in_flight.saturating_sub(1);
        }

        let mut next_requests = Vec::new();
        while *in_flight < limit {
            let Some(request) = queue.pop_next(*low_in_flight, low_limit) else {
                break;
            };
            *in_flight += 1;
            if request.priority == ImagePriority::Low {
                *low_in_flight += 1;
            }
            next_requests.push(request);
        }

        next_requests
    }

    fn spawn_request(async_cx: AsyncApp, request: QueuedImageRequest) {
        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                let request_started = Instant::now();
                let bucket = request.bucket;
                let priority = request.priority;
                let key = request.key.clone();
                let fallback_mimetype = match &request.request {
                    ImageRequestKind::MediaSource { mimetype, .. } => mimetype.clone(),
                    ImageRequestKind::Url { .. } => None,
                };
                let is_avatar = matches!(
                    &request.request,
                    ImageRequestKind::Url {
                        is_avatar: true,
                        ..
                    }
                );

                let (result, from_disk) = if let Some(bytes) =
                    Self::load_disk_cached_bytes(bucket, &request.key)
                {
                    (Ok((bytes, fallback_mimetype.clone())), true)
                } else {
                    let result = match request.request {
                        ImageRequestKind::Url { url, is_avatar } => {
                            Self::fetch_image_bytes(&request.client, &url, is_avatar).await
                        }
                        ImageRequestKind::MediaSource { source, .. } => {
                            if let Some(client) = &request.client {
                                let mxc_str = match &source {
                                    MediaSource::Plain(mxc) => mxc.to_string(),
                                    MediaSource::Encrypted(enc) => enc.url.to_string(),
                                };
                                Self::fetch_from_matrix_media(client, source, false, mxc_str).await
                            } else {
                                Err(anyhow::anyhow!("Client not available"))
                            }
                        }
                    };
                    (result, false)
                };

                Self::process_image_result(
                    result,
                    key,
                    bucket,
                    is_avatar,
                    fallback_mimetype,
                    request_started,
                    async_cx,
                    from_disk,
                    priority,
                )
                .await;
            })
            .detach();
    }

    async fn process_image_result(
        result: anyhow::Result<(Vec<u8>, Option<String>)>,
        url_clone: String,
        bucket: ImageBucket,
        is_avatar: bool,
        fallback_mimetype: Option<String>,
        started: Instant,
        async_cx: AsyncApp,
        from_disk: bool,
        priority: ImagePriority,
    ) {
        let bucket_label = match bucket {
            ImageBucket::Avatar => "avatar",
            ImageBucket::Media => "media",
        };
        let source_label = if from_disk { "disk" } else { "network" };

        match result {
            Ok((bytes, content_type)) => {
                let mut bytes = bytes.to_vec();
                let mut content_type = content_type.or(fallback_mimetype);
                let original_bytes_len = bytes.len();

                if is_avatar
                    && !from_disk
                    && let Some(processed) = Self::prepare_avatar_bytes(&bytes)
                {
                    bytes = processed;
                    content_type = Some("image/png".to_string());
                }

                if let Some(format) = Self::detect_format(&bytes, content_type.as_deref()) {
                    if !from_disk {
                        Self::store_disk_cached_bytes(bucket, &url_clone, &bytes);
                    }
                    let image = Arc::new(Image::from_bytes(format, bytes));
                    let mut notify_views = Vec::new();
                    let mut next_requests = Vec::new();
                    let _ = async_cx.update(|cx: &mut App| {
                        cx.update_global::<Self, _>(|this, _cx| {
                            notify_views = this.store_success(bucket, url_clone.clone(), image);
                            next_requests = this.complete_request(bucket, priority);
                        });
                    });
                    Self::schedule_notify(async_cx.clone(), notify_views);
                    for request in next_requests {
                        Self::spawn_request(async_cx.clone(), request);
                    }
                    perf::log_if_slow("image.request", started, Duration::from_millis(16), || {
                        format!(
                            "bucket={bucket_label} avatar={} ok=true source={} source_bytes={} stored_format={format:?} key={url_clone}",
                            is_avatar, source_label, original_bytes_len,
                        )
                    });
                } else if let Some(png_bytes) = Self::decode_to_png_bytes(&bytes) {
                    if !from_disk {
                        Self::store_disk_cached_bytes(bucket, &url_clone, &png_bytes);
                    }
                    let image = Arc::new(Image::from_bytes(gpui::ImageFormat::Png, png_bytes));
                    let mut notify_views = Vec::new();
                    let mut next_requests = Vec::new();
                    let _ = async_cx.update(|cx: &mut App| {
                        cx.update_global::<Self, _>(|this, _cx| {
                            notify_views = this.store_success(bucket, url_clone.clone(), image);
                            next_requests = this.complete_request(bucket, priority);
                        });
                    });
                    Self::schedule_notify(async_cx.clone(), notify_views);
                    for request in next_requests {
                        Self::spawn_request(async_cx.clone(), request);
                    }
                    perf::log_if_slow("image.request", started, Duration::from_millis(16), || {
                        format!(
                            "bucket={bucket_label} avatar={} ok=true source={} source_bytes={} stored_format=png-decoded key={url_clone}",
                            is_avatar, source_label, original_bytes_len,
                        )
                    });
                } else {
                    tracing::error!(
                        "Unknown image format for {}. Content-Type: {:?}",
                        url_clone,
                        content_type
                    );
                    let mut notify_views = Vec::new();
                    let mut next_requests = Vec::new();
                    let _ = async_cx.update(|cx: &mut App| {
                        cx.update_global::<Self, _>(|this, _cx| {
                            notify_views = this.store_failure(
                                bucket,
                                url_clone.clone(),
                                Duration::from_secs(30),
                            );
                            next_requests = this.complete_request(bucket, priority);
                        });
                    });
                    Self::schedule_notify(async_cx.clone(), notify_views);
                    for request in next_requests {
                        Self::spawn_request(async_cx.clone(), request);
                    }
                    perf::log_if_slow("image.request", started, Duration::from_millis(16), || {
                        format!(
                            "bucket={bucket_label} avatar={} ok=false source={} source_bytes={} error=unknown-format key={url_clone}",
                            is_avatar, source_label, original_bytes_len,
                        )
                    });
                }
            }
            Err(e) => {
                tracing::error!("Failed to fetch image {}: {:?}", url_clone, e);
                let mut notify_views = Vec::new();
                let mut next_requests = Vec::new();
                let _ = async_cx.update(|cx: &mut App| {
                    cx.update_global::<Self, _>(|this, _cx| {
                        notify_views =
                            this.store_failure(bucket, url_clone.clone(), Duration::from_secs(10));
                        next_requests = this.complete_request(bucket, priority);
                    });
                });
                Self::schedule_notify(async_cx.clone(), notify_views);
                for request in next_requests {
                    Self::spawn_request(async_cx.clone(), request);
                }
                perf::log_if_slow("image.request", started, Duration::from_millis(16), || {
                    format!(
                        "bucket={bucket_label} avatar={} ok=false source={} error={e:?} key={url_clone}",
                        is_avatar, source_label
                    )
                });
            }
        }
    }

    fn schedule_notify(async_cx: AsyncApp, views: Vec<EntityId>) {
        if views.is_empty() {
            return;
        }

        let mut should_schedule = false;
        let _ = async_cx.update(|cx: &mut App| {
            cx.update_global::<Self, _>(|this, _cx| {
                this.pending_notify_views.extend(views.iter().copied());
                if !this.notify_scheduled {
                    this.notify_scheduled = true;
                    should_schedule = true;
                }
            });
        });

        if !should_schedule {
            return;
        }

        let timer_executor = async_cx.background_executor().clone();
        let refresh_cx = async_cx.clone();
        async_cx
            .clone()
            .spawn(move |_: &mut AsyncApp| async move {
                timer_executor
                    .timer(Duration::from_millis(IMAGE_NOTIFY_BATCH_MS))
                    .await;
                let _ = refresh_cx.update(|cx: &mut App| {
                    let mut pending_views = Vec::new();
                    cx.update_global::<Self, _>(|this, _cx| {
                        this.notify_scheduled = false;
                        pending_views.extend(this.pending_notify_views.drain());
                    });
                    for view_id in pending_views {
                        cx.notify(view_id);
                    }
                });
            })
            .detach();
    }

    fn store_success(
        &mut self,
        bucket: ImageBucket,
        key: String,
        image: Arc<Image>,
    ) -> Vec<EntityId> {
        match bucket {
            ImageBucket::Avatar => {
                self.avatar_cache.insert(key.clone(), image);
                self.avatar_failed_until.remove(&key);
                self.avatar_pending.remove(&key);
                self.avatar_waiters
                    .remove(&key)
                    .map(|waiters| waiters.into_iter().collect())
                    .unwrap_or_default()
            }
            ImageBucket::Media => {
                self.media_cache.insert(key.clone(), image);
                self.media_failed_until.remove(&key);
                self.media_pending.remove(&key);
                self.media_waiters
                    .remove(&key)
                    .map(|waiters| waiters.into_iter().collect())
                    .unwrap_or_default()
            }
        }
    }

    fn store_failure(
        &mut self,
        bucket: ImageBucket,
        key: String,
        delay: Duration,
    ) -> Vec<EntityId> {
        match bucket {
            ImageBucket::Avatar => {
                self.avatar_failed_until
                    .insert(key.clone(), Instant::now() + delay);
                self.avatar_pending.remove(&key);
                self.avatar_waiters
                    .remove(&key)
                    .map(|waiters| waiters.into_iter().collect())
                    .unwrap_or_default()
            }
            ImageBucket::Media => {
                self.media_failed_until
                    .insert(key.clone(), Instant::now() + delay);
                self.media_pending.remove(&key);
                self.media_waiters
                    .remove(&key)
                    .map(|waiters| waiters.into_iter().collect())
                    .unwrap_or_default()
            }
        }
    }

    async fn fetch_image_bytes(
        client: &Option<RivetClient>,
        url: &str,
        is_avatar: bool,
    ) -> anyhow::Result<(Vec<u8>, Option<String>)> {
        let mxc_uri = if url.starts_with("mxc://") {
            Some(url.to_string())
        } else if let Some(pos) = url.find("/_matrix/media/v3/download/") {
            let mxc_part = &url[pos + "/_matrix/media/v3/download/".len()..];
            Some(format!("mxc://{}", mxc_part))
        } else if let Some(pos) = url.find("/_matrix/media/v3/thumbnail/") {
            let mxc_part = &url[pos + "/_matrix/media/v3/thumbnail/".len()..];
            let clean_mxc = mxc_part.split('?').next().unwrap_or(mxc_part);
            Some(format!("mxc://{}", clean_mxc))
        } else {
            None
        };

        if let (Some(client), Some(mxc_str)) = (client, mxc_uri) {
            let source = MediaSource::Plain(matrix_sdk::ruma::OwnedMxcUri::from(mxc_str.clone()));
            Self::fetch_from_matrix_media(client, source, is_avatar, mxc_str).await
        } else {
            Self::fetch_with_reqwest(url).await
        }
    }

    async fn fetch_from_matrix_media(
        client: &RivetClient,
        source: MediaSource,
        is_avatar: bool,
        mxc_str: String,
    ) -> anyhow::Result<(Vec<u8>, Option<String>)> {
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

        let bytes = client
            .client()
            .media()
            .get_media_content(&file, true)
            .await
            .map_err(|e| anyhow::anyhow!(e))?
            .to_vec();

        if Self::detect_format(&bytes, None).is_some() {
            return Ok((bytes, None));
        }

        let direct_url = client.resolve_mxc(&mxc_str);
        match Self::fetch_with_reqwest(&direct_url).await {
            Ok((direct_bytes, direct_content_type))
                if Self::detect_format(&direct_bytes, direct_content_type.as_deref()).is_some() =>
            {
                Ok((direct_bytes, direct_content_type))
            }
            Ok(_) => Ok((bytes, None)),
            Err(err) => {
                tracing::debug!("Direct media fetch fallback failed for {mxc_str}: {err:?}");
                Ok((bytes, None))
            }
        }
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

    fn disk_cache_root(bucket: ImageBucket) -> PathBuf {
        let bucket_name = match bucket {
            ImageBucket::Avatar => "avatars",
            ImageBucket::Media => "media",
        };
        store::data_root().join("image-cache").join(bucket_name)
    }

    fn disk_cache_path(bucket: ImageBucket, key: &str) -> PathBuf {
        Self::disk_cache_root(bucket).join(format!("{:016x}.bin", Self::fnv1a64(key.as_bytes())))
    }

    fn fnv1a64(bytes: &[u8]) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    fn load_disk_cached_bytes(bucket: ImageBucket, key: &str) -> Option<Vec<u8>> {
        std::fs::read(Self::disk_cache_path(bucket, key)).ok()
    }

    fn should_persist(bucket: ImageBucket, len: usize) -> bool {
        match bucket {
            ImageBucket::Avatar => true,
            ImageBucket::Media => len <= 2 * 1024 * 1024,
        }
    }

    fn store_disk_cached_bytes(bucket: ImageBucket, key: &str, bytes: &[u8]) {
        if !Self::should_persist(bucket, bytes.len()) {
            return;
        }

        let path = Self::disk_cache_path(bucket, key);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, bytes);
    }

    fn prepare_avatar_bytes(input: &[u8]) -> Option<Vec<u8>> {
        let image = image::load_from_memory(input).ok()?;
        let mut rgba = image.to_rgba8();
        let (mut width, mut height) = rgba.dimensions();

        if width == 0 || height == 0 {
            return None;
        }

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

    fn detect_format(bytes: &[u8], content_type: Option<&str>) -> Option<gpui::ImageFormat> {
        let from_content_type = content_type.and_then(gpui::ImageFormat::from_mime_type);
        if from_content_type.is_some() {
            return from_content_type;
        }

        if let Ok(format) = image::guess_format(bytes) {
            let mapped = match format {
                ImageFormat::Png => Some(gpui::ImageFormat::Png),
                ImageFormat::Jpeg => Some(gpui::ImageFormat::Jpeg),
                ImageFormat::Gif => Some(gpui::ImageFormat::Gif),
                ImageFormat::WebP => Some(gpui::ImageFormat::Webp),
                ImageFormat::Bmp => Some(gpui::ImageFormat::Bmp),
                ImageFormat::Tiff => Some(gpui::ImageFormat::Tiff),
                _ => None,
            };

            if mapped.is_some() {
                return mapped;
            }
        }

        let sniff_len = bytes.len().min(4096);
        if let Ok(sample) = std::str::from_utf8(&bytes[..sniff_len]) {
            let sample = sample.trim_start_matches('\u{feff}');
            if sample.contains("<svg") {
                return Some(gpui::ImageFormat::Svg);
            }
        }

        None
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

pub(crate) fn prepare_avatar_bytes_for_bench(input: &[u8]) -> Option<Vec<u8>> {
    ImageCache::prepare_avatar_bytes(input)
}
