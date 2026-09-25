fn main() {
    // Every command the frontend may invoke must be listed here. Tauri generates an
    // `allow-<command>` permission for each, and capabilities/main-window.json grants
    // them explicitly. A command missing from the capability cannot be invoked.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "get_session",
        "enter_work_mode",
        "leave_work_mode",
        "get_workspace",
        "list_drafts",
        "get_draft",
        "create_draft",
        "save_draft",
        "delete_draft",
        "parse_pasted_rows",
        "assess_draft",
        "review_draft",
        "complete_draft",
        "list_recent_investigations",
        "search_investigations",
        "get_investigation",
        "compose_distribution",
        "distribute_investigation",
        "export_investigation_pdf",
        "export_draft_pdf",
        "admin_get_configuration",
        "admin_save_system",
        "admin_set_system_active",
        "admin_save_station",
        "admin_set_station_active",
        "admin_save_section",
        "admin_set_section_active",
        "admin_move_section",
        "admin_save_mail_template",
        "admin_save_publication",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest)).expect("failed to run tauri-build");
}
