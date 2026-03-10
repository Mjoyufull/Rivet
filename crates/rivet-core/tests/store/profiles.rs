use rivet_core::store::{profile_id, profile_paths, profile_paths_for_session};

#[test]
fn profile_id_is_deterministic_and_sanitized() {
    let first = profile_id("https://matrix.org", "@Wired Wireless:matrix.org");
    let second = profile_id("https://matrix.org", "@Wired Wireless:matrix.org");

    assert_eq!(first, second);
    assert!(first.starts_with("matrix.org-wired-wireless-"));
    assert!(
        first
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '.')
    );
}

#[test]
fn profile_paths_use_profile_root_structure() {
    let paths = profile_paths("example-profile");

    assert_eq!(paths.profile_id, "example-profile");
    assert!(paths.root_dir.ends_with("profiles/example-profile"));
    assert!(
        paths
            .store_path
            .ends_with("profiles/example-profile/matrix-sdk")
    );
    assert!(
        paths
            .session_path
            .ends_with("profiles/example-profile/session.json")
    );
}

#[test]
fn profile_paths_for_session_uses_derived_profile_id() {
    let derived = profile_id("https://chat.example.org", "@alice:example.org");
    let paths = profile_paths_for_session("https://chat.example.org", "@alice:example.org");

    assert_eq!(paths.profile_id, derived);
    assert!(paths.root_dir.ends_with(format!("profiles/{derived}")));
}
