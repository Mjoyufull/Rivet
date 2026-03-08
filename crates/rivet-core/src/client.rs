use crate::store::{self, ProfilePaths, SessionData};
use anyhow::{Context, Result};
use matrix_sdk::Client;
use matrix_sdk::ruma::api::client::session::get_login_types::v3::LoginType;
use matrix_sdk::store::RoomLoadSettings;
use matrix_sdk_ui::room_list_service::RoomListService;
use matrix_sdk_ui::sync_service::SyncService;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct RivetClient {
    homeserver_url: String,
    profile_paths: Option<ProfilePaths>,
    client: Arc<Client>,
    room_list_service: Arc<RwLock<Option<Arc<RoomListService>>>>,
    sync_service: Arc<RwLock<Option<Arc<SyncService>>>>,
}

impl RivetClient {
    pub fn homeserver_url(&self) -> &str {
        &self.homeserver_url
    }

    pub fn resolve_mxc(&self, mxc_url: &str) -> String {
        if let Some(mxc) = mxc_url.strip_prefix("mxc://") {
            format!("{}/_matrix/media/v3/download/{}", self.homeserver_url, mxc)
        } else {
            mxc_url.to_string()
        }
    }
    pub async fn new(homeserver_url: &str) -> Result<Self> {
        let homeserver_url = homeserver_url.trim();
        let normalized_url = Self::normalize_url(homeserver_url);

        tracing::info!("Initializing RivetClient for: '{}'", normalized_url);
        let client = Self::create_client(&normalized_url).await?;

        Ok(Self::from_client(normalized_url, None, client))
    }

    fn normalize_url(url: &str) -> String {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            format!("https://{}", url)
        } else {
            url.to_string()
        }
    }

    async fn create_client(normalized_url: &str) -> Result<Client> {
        Client::builder()
            .server_name_or_homeserver_url(normalized_url)
            .build()
            .await
            .context(format!(
                "Failed to build matrix client for homeserver: {}",
                normalized_url
            ))
    }

    async fn create_persistent_client(
        normalized_url: &str,
        profile_paths: &ProfilePaths,
    ) -> Result<Client> {
        store::ensure_profile_dirs(profile_paths)?;

        Client::builder()
            .server_name_or_homeserver_url(normalized_url)
            .sqlite_store(&profile_paths.store_path, None)
            .build()
            .await
            .context(format!(
                "Failed to build persistent matrix client for homeserver: {}",
                normalized_url
            ))
    }

    fn from_client(
        homeserver_url: String,
        profile_paths: Option<ProfilePaths>,
        client: Client,
    ) -> Self {
        Self {
            homeserver_url,
            profile_paths,
            client: Arc::new(client),
            room_list_service: Arc::new(RwLock::new(None)),
            sync_service: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn restore() -> Result<Option<Self>> {
        let profile_paths = if let Some(profile_id) = store::load_active_profile()? {
            store::profile_paths(&profile_id)
        } else if let Some(paths) = store::migrate_legacy_session_if_needed()? {
            paths
        } else {
            return Ok(None);
        };

        let session_data = match store::load_session(&profile_paths) {
            Ok(data) => data,
            Err(error) => {
                tracing::warn!(
                    "Failed to load saved session for profile {}: {:?}",
                    profile_paths.profile_id,
                    error
                );
                store::clear_active_profile().ok();
                return Ok(None);
            }
        };

        tracing::info!(
            "Restoring session for {} at {}",
            session_data.session.meta.user_id,
            session_data.homeserver_url
        );

        let client =
            Self::create_persistent_client(&session_data.homeserver_url, &profile_paths).await?;
        match client
            .matrix_auth()
            .restore_session(session_data.session.clone(), RoomLoadSettings::default())
            .await
        {
            Ok(_) => {
                let rivet_client =
                    Self::from_client(session_data.homeserver_url, Some(profile_paths), client);

                rivet_client.init_services().await?;
                Ok(Some(rivet_client))
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to restore session, deleting stale session data: {:?}",
                    e
                );
                std::fs::remove_file(&profile_paths.session_path).ok();
                store::clear_active_profile().ok();
                Ok(None)
            }
        }
    }

    async fn save_session(&self) -> Result<()> {
        let profile_paths = self
            .profile_paths
            .as_ref()
            .context("Cannot save a session for a temporary client")?;
        if let Some(session) = self.client.matrix_auth().session() {
            let data = SessionData {
                homeserver_url: self.homeserver_url.clone(),
                session,
            };
            store::save_session(profile_paths, &data)?;
            store::save_active_profile(&profile_paths.profile_id)?;
            tracing::info!(
                "Session saved for profile {} at {}",
                profile_paths.profile_id,
                profile_paths.session_path.display()
            );
        }
        Ok(())
    }

    async fn finalize_logged_in_client(&self) -> Result<Self> {
        let session = self
            .client
            .matrix_auth()
            .session()
            .context("No session available after login")?;
        let profile_paths =
            store::profile_paths_for_session(&self.homeserver_url, session.meta.user_id.as_str());

        tracing::info!(
            "Finalizing authenticated profile {} for {}",
            profile_paths.profile_id,
            session.meta.user_id
        );

        let client = Self::create_persistent_client(&self.homeserver_url, &profile_paths).await?;
        client
            .matrix_auth()
            .restore_session(session, RoomLoadSettings::default())
            .await
            .context("Failed to restore authenticated session into persistent store")?;

        let rivet_client = Self::from_client(
            self.homeserver_url.clone(),
            Some(profile_paths.clone()),
            client,
        );
        rivet_client.save_session().await?;
        rivet_client.init_services().await?;
        Ok(rivet_client)
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<Self> {
        tracing::info!("Attempting password login for user: {}", username);

        self.client
            .matrix_auth()
            .login_username(username, password)
            .initial_device_display_name("Rivet Matrix Client")
            .send()
            .await
            .context("Login failed")?;

        tracing::info!("Password login successful for {}", username);
        self.finalize_logged_in_client().await
    }

    /// Get the SSO login URL for browser-based authentication.
    pub async fn get_sso_login_url(&self, redirect_url: &str) -> Result<String> {
        tracing::info!("Requesting SSO login types from homeserver");
        let login_types = self
            .client
            .matrix_auth()
            .get_login_types()
            .await
            .context("Failed to get login types")?;

        // Check if SSO is available
        let has_sso = login_types
            .flows
            .iter()
            .any(|f| matches!(f, LoginType::Sso(_)));
        if !has_sso {
            tracing::error!("SSO login is not available on this homeserver");
            anyhow::bail!("SSO login is not available on this homeserver");
        }

        tracing::info!("Building SSO login URL with redirect: {}", redirect_url);
        let url = self
            .client
            .matrix_auth()
            .get_sso_login_url(redirect_url, None)
            .await
            .context("Failed to get SSO login URL")?;

        Ok(url.to_string())
    }

    /// Complete SSO login after browser redirect.
    pub async fn complete_sso_login(&self, callback_url: &str) -> Result<Self> {
        tracing::info!("Completing SSO login with callback: {}", callback_url);

        // Parse the callback URL
        let url = url::Url::parse(callback_url).context("Invalid callback URL")?;

        // Check for loginToken
        let token = url
            .query_pairs()
            .find(|(k, _)| k == "loginToken")
            .map(|(_, v)| v.to_string());
        if let Some(token) = token {
            tracing::info!("Found loginToken in callback: {} characters", token.len());
        } else {
            tracing::warn!("No loginToken found in callback URL!");
        }

        // Use the SDK's login_with_sso_callback which handles the full URL
        tracing::info!("Calling login_with_sso_callback...");
        let login_future = self
            .client
            .matrix_auth()
            .login_with_sso_callback(url)
            .context("Failed to start SSO callback login")?
            .initial_device_display_name("Rivet Matrix Client");

        match login_future.await {
            Ok(_) => {
                tracing::info!("SSO login succeeded, finalizing persistent profile");
                self.finalize_logged_in_client().await
            }
            Err(e) => Err(e).context("SSO login failed in .await"),
        }
    }

    /// Initialize room list and sync services after login.
    async fn init_services(&self) -> Result<()> {
        let client_handle = (*self.client).clone();

        tracing::info!("Initializing SyncService");
        let sync = SyncService::builder(client_handle)
            .build()
            .await
            .context("Failed to initialize SyncService")?;
        let sync = Arc::new(sync);

        tracing::info!("Getting RoomListService from SyncService");
        let room_list = sync.room_list_service();

        tracing::info!("Spawning SyncService background loop");
        let sync_clone = sync.clone();
        tokio::spawn(async move {
            tracing::info!("SyncService background loop started");

            // Monitor sync service state
            let mut state_stream = sync_clone.state();
            tokio::spawn(async move {
                while let Some(state) = state_stream.next().await {
                    tracing::info!("SyncService state changed to: {:?}", state);
                }
            });

            sync_clone.start().await;
            tracing::info!("SyncService background loop started successfully");
        });

        *self.room_list_service.write().await = Some(room_list);
        *self.sync_service.write().await = Some(sync);

        tracing::info!("Rivet services initialized and sync spawned");
        Ok(())
    }

    pub fn client(&self) -> Arc<Client> {
        self.client.clone()
    }

    pub async fn room_list_service(&self) -> Option<Arc<RoomListService>> {
        self.room_list_service.read().await.clone()
    }

    pub async fn sync_service(&self) -> Option<Arc<SyncService>> {
        self.sync_service.read().await.clone()
    }

    pub async fn shutdown(&self) -> Result<()> {
        if let Some(sync_service) = self.sync_service().await {
            sync_service.stop().await;
        }

        *self.sync_service.write().await = None;
        *self.room_list_service.write().await = None;

        Ok(())
    }

    /// Get the user ID of the current logged-in user
    pub fn user_id(&self) -> Option<String> {
        self.client.user_id().map(|id| id.to_string())
    }

    /// Get the display name of the current logged-in user
    pub async fn display_name(&self) -> Option<String> {
        if let Some(_user_id) = self.client.user_id() {
            if let Ok(Some(response)) = self.client.account().get_display_name().await {
                return Some(response);
            }
        }
        None
    }

    /// Get the avatar URL of the current logged-in user
    pub async fn avatar_url(&self) -> Option<String> {
        if let Some(_user_id) = self.client.user_id() {
            if let Ok(Some(response)) = self.client.account().get_avatar_url().await {
                return Some(response.to_string());
            }
        }
        None
    }

    pub async fn logout(&self) -> Result<()> {
        tracing::info!("Logging out user");
        self.shutdown().await?;

        if let Some(profile_paths) = &self.profile_paths {
            if profile_paths.session_path.exists() {
                std::fs::remove_file(&profile_paths.session_path).with_context(|| {
                    format!(
                        "Failed to remove session file {}",
                        profile_paths.session_path.display()
                    )
                })?;
            }

            if store::load_active_profile()?.as_deref() == Some(profile_paths.profile_id.as_str()) {
                store::clear_active_profile()?;
            }
        }

        // We could also try to call client.matrix_auth().logout() if we wanted to invalidate the token on the server,
        // but for now, let's just clear the local session.
        // self.client.matrix_auth().logout().await?;

        Ok(())
    }

    pub fn delete_all_local_data() -> Result<()> {
        store::delete_all_data()
    }
}
