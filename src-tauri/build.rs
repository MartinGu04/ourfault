fn main() {
    // Every command the frontend may invoke must be listed here. Tauri generates an
    // `allow-<command>` permission for each, and capabilities/main-window.json grants
    // them explicitly. A command missing from the capability cannot be invoked.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "get_session",
        "list_recent_investigations",
        "find_investigation",
        "parse_pasted_rows",
        "list_active_systems",
        "peek_next_investigation_number",
        "preview_investigation",
        "create_investigation",
        "compose_distribution",
        "distribute_investigation",
        "admin_list_systems",
        "admin_save_system",
        "admin_set_system_active",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest)).expect("failed to run tauri-build");
}
