use anyhow::{Context, Result};
use matrix_sdk::authentication::matrix::MatrixSession;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub homeserver_url: String,
    pub session: MatrixSession,
}

#[derive(Clone, Serialize, Deserialize)]
struct ActiveProfile {
    profile_id: String,
}

#[derive(Clone, Debug)]
pub struct ProfilePaths {
    pub profile_id: String,
    pub root_dir: PathBuf,
    pub store_path: PathBuf,
    pub session_path: PathBuf,
}

pub fn data_root() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("Rivet");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Rivet");
        }
    }

    if let Some(xdg_data_home) = env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(xdg_data_home).join("rivet");
    }

    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("rivet");
    }

    PathBuf::from("rivet-store")
}

fn legacy_data_root() -> PathBuf {
    PathBuf::from("rivet-store")
}

fn profiles_root() -> PathBuf {
    data_root().join("profiles")
}

fn active_profile_path() -> PathBuf {
    data_root().join("active_profile.json")
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }
    Ok(())
}

fn sanitize_label(input: &str) -> String {
    let mut sanitized = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_' | '.') {
            sanitized.push(ch);
        } else if !sanitized.ends_with('-') {
            sanitized.push('-');
        }
    }

    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "profile".to_string()
    } else {
        trimmed.chars().take(32).collect()
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn profile_id(homeserver_url: &str, user_id: &str) -> String {
    let host = url::Url::parse(homeserver_url)
        .ok()
        .and_then(|url| url.host_str().map(sanitize_label))
        .unwrap_or_else(|| "matrix".to_string());
    let user = sanitize_label(
        user_id
            .trim_start_matches('@')
            .split(':')
            .next()
            .unwrap_or(user_id),
    );
    let hash = fnv1a64(format!("{homeserver_url}\n{user_id}").as_bytes());

    format!("{host}-{user}-{hash:016x}")
}

pub fn profile_paths(profile_id: &str) -> ProfilePaths {
    let root_dir = profiles_root().join(profile_id);
    ProfilePaths {
        profile_id: profile_id.to_string(),
        store_path: root_dir.join("matrix-sdk"),
        session_path: root_dir.join("session.json"),
        root_dir,
    }
}

pub fn profile_paths_for_session(homeserver_url: &str, user_id: &str) -> ProfilePaths {
    profile_paths(&profile_id(homeserver_url, user_id))
}

pub fn ensure_profile_dirs(paths: &ProfilePaths) -> Result<()> {
    fs::create_dir_all(&paths.root_dir)
        .with_context(|| format!("Failed to create profile dir {}", paths.root_dir.display()))?;
    Ok(())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    ensure_parent_dir(path)?;
    let json = serde_json::to_string(value)?;
    fs::write(path, json).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let json =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let value = serde_json::from_str(&json)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(value)
}

pub fn save_session(paths: &ProfilePaths, data: &SessionData) -> Result<()> {
    ensure_profile_dirs(paths)?;
    write_json(&paths.session_path, data)
}

pub fn load_session(paths: &ProfilePaths) -> Result<SessionData> {
    read_json(&paths.session_path)
}

pub fn save_active_profile(profile_id: &str) -> Result<()> {
    let active = ActiveProfile {
        profile_id: profile_id.to_string(),
    };
    write_json(&active_profile_path(), &active)
}

pub fn load_active_profile() -> Result<Option<String>> {
    let path = active_profile_path();
    if !path.exists() {
        return Ok(None);
    }

    let active: ActiveProfile = read_json(&path)?;
    Ok(Some(active.profile_id))
}

pub fn clear_active_profile() -> Result<()> {
    let path = active_profile_path();
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("Failed to remove {}", path.display()))?;
    }
    Ok(())
}

pub fn migrate_legacy_session_if_needed() -> Result<Option<ProfilePaths>> {
    if load_active_profile()?.is_some() {
        return Ok(None);
    }

    let legacy_session_path = legacy_data_root().join("session.json");
    if !legacy_session_path.exists() {
        return Ok(None);
    }

    let session_data: SessionData = read_json(&legacy_session_path)?;
    let paths = profile_paths_for_session(
        &session_data.homeserver_url,
        session_data.session.meta.user_id.as_str(),
    );
    save_session(&paths, &session_data)?;
    save_active_profile(&paths.profile_id)?;
    Ok(Some(paths))
}

pub fn delete_all_data() -> Result<()> {
    let root = data_root();
    if root.exists() {
        fs::remove_dir_all(&root)
            .with_context(|| format!("Failed to remove {}", root.display()))?;
    }

    let legacy_root = legacy_data_root();
    if legacy_root != root && legacy_root.exists() {
        fs::remove_dir_all(&legacy_root)
            .with_context(|| format!("Failed to remove {}", legacy_root.display()))?;
    }

    Ok(())
}
