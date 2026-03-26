use super::super::pool_lookup::select_library_pool_path;

#[test]
fn select_library_pool_path_prefers_explicit_library_path() {
    let path = "/config/Library/Application Support/Plex Media Server/Plug-in Support/Databases/com.plexapp.plugins.library.db";
    assert_eq!(select_library_pool_path(path, None), Some(path.to_string()));
}

#[test]
fn select_library_pool_path_uses_cached_library_path_for_empty_input() {
    let cached = "/config/Library/Application Support/Plex Media Server/Plug-in Support/Databases/com.plexapp.plugins.library.db";
    assert_eq!(
        select_library_pool_path("", Some(cached)),
        Some(cached.to_string())
    );
}

#[test]
fn select_library_pool_path_maps_blobs_path_to_library_path() {
    let blobs = "/config/Library/Application Support/Plex Media Server/Plug-in Support/Databases/com.plexapp.plugins.library.blobs.db";
    let library = "/config/Library/Application Support/Plex Media Server/Plug-in Support/Databases/com.plexapp.plugins.library.db";
    assert_eq!(
        select_library_pool_path(blobs, None),
        Some(library.to_string())
    );
}

#[test]
fn select_library_pool_path_prefers_cached_library_path_for_blobs_input() {
    let blobs = "/config/Library/Application Support/Plex Media Server/Plug-in Support/Databases/com.plexapp.plugins.library.blobs.db";
    let cached = "/other/config/com.plexapp.plugins.library.db";
    assert_eq!(
        select_library_pool_path(blobs, Some(cached)),
        Some(cached.to_string())
    );
}

#[test]
fn select_library_pool_path_rejects_non_library_input() {
    let cached = "/config/Library/Application Support/Plex Media Server/Plug-in Support/Databases/com.plexapp.plugins.library.db";
    assert_eq!(
        select_library_pool_path(
            "/config/Library/Application Support/Plex Media Server/Plug-in Support/Databases/other.db",
            Some(cached)
        ),
        None
    );
}
