mod claude_md;
mod codex_oauth;
mod commands;
mod copilot_auth;
mod db;
mod deeplink;
mod error;
mod gemini_transform;
mod grok_config;
mod hermes;
mod hooks;
mod lightweight;
mod mcp;
mod omo;
mod openclaw_config;
mod provider_proxy;
mod provider_proxy_transform;
mod proxy_optimizer;
mod s3_sync;
mod security;
mod shared;
mod skills;
mod tray;
mod updater;
mod url_logging;
mod utils;
mod webdav_sync;
mod window_preferences;
mod window_runtime;
mod workflows;
mod xai_oauth;
use commands::compat_commands;
use commands::extra_commands;
use commands::skill_commands;
use tauri::{tray::TrayIconEvent, AppHandle, Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use url_logging::redact_url_for_log;
use window_preferences::load_window_preferences;
use window_runtime::{configure_main_window, show_or_create_main_window};
fn focus_main_window(app_handle: &AppHandle) {
    show_or_create_main_window(app_handle, true);
}
fn handle_deeplink_url(
    app_handle: &AppHandle,
    url_str: &str,
    focus_window: bool,
    source: &str,
) -> bool {
    if !url_str.starts_with("cchub://") {
        return false;
    }
    let redacted_url = redact_url_for_log(url_str);
    utils::append_runtime_log(
        "info",
        "deeplink",
        &format!("Received deep link from {source}: {redacted_url}"),
    );
    match deeplink::parse_deeplink_url(url_str) {
        Ok(request) => {
            if let Some(state) = app_handle.try_state::<deeplink::DeepLinkState>() {
                if let Err(error) = state.enqueue_import(request.clone()) {
                    utils::append_runtime_log(
                        "error",
                        "deeplink",
                        &format!("Failed to queue deep link import: {error}"),
                    );
                }
            }
            if let Err(error) = app_handle.emit("deeplink-import", &request) {
                utils::append_runtime_log(
                    "error",
                    "deeplink",
                    &format!("Failed to emit deep link import event: {error}"),
                );
            }
            if focus_window {
                focus_main_window(app_handle);
            }
        }
        Err(error) => {
            let payload = deeplink::DeepLinkErrorPayload {
                url: redacted_url.clone(),
                error: error.to_string(),
            };
            if let Some(state) = app_handle.try_state::<deeplink::DeepLinkState>() {
                if let Err(queue_error) = state.enqueue_error(payload.clone()) {
                    utils::append_runtime_log(
                        "error",
                        "deeplink",
                        &format!("Failed to queue deep link error: {queue_error}"),
                    );
                }
            }
            if let Err(emit_error) = app_handle.emit("deeplink-error", &payload) {
                utils::append_runtime_log(
                    "error",
                    "deeplink",
                    &format!("Failed to emit deep link error event: {emit_error}"),
                );
            }
        }
    }
    true
}
pub fn run() {
    utils::install_panic_hook();
    let mut builder = tauri::Builder::default()
        .manage(deeplink::DeepLinkState::default())
        .manage(commands::autopilot_commands::AutopilotRuntime::default());
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let mut found_deeplink = false;
            for arg in &argv {
                if handle_deeplink_url(app, arg, true, "single-instance") {
                    found_deeplink = true;
                    break;
                }
            }
            if !found_deeplink {
                focus_main_window(app);
            }
        }));
    }
    builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            db::init_db(&app_handle)?;
            if let Ok(mut conn) = app_handle.state::<db::DbState>().0.lock() {
                if let Err(error) =
                    commands::model_pricing_file::sync_local_model_pricing(&mut conn)
                {
                    utils::append_runtime_log(
                        "warn",
                        "usage",
                        &format!("Failed to restore local model pricing overrides: {error}"),
                    );
                }
                extra_commands::ensure_official_config_profiles_seeded(&conn)
                    .map_err(std::io::Error::other)?;
                if let Err(error) = webdav_sync::migrate_webdav_password_to_keyring(&conn) {
                    utils::append_runtime_log(
                        "warn",
                        "webdav",
                        &format!("Failed to migrate WebDAV password to keyring: {error}"),
                    );
                }
                if let Err(error) = s3_sync::migrate_secret_to_keyring(&conn) {
                    utils::append_runtime_log(
                        "warn",
                        "s3",
                        &format!("Failed to migrate S3 secret to keyring: {error}"),
                    );
                }
            }
            copilot_auth::init_copilot_auth_state(&app_handle);
            codex_oauth::init_codex_oauth_state(&app_handle);
            xai_oauth::init_xai_oauth_state(&app_handle);
            provider_proxy::init_local_provider_proxy_runtime(&app_handle);
            if let Ok(conn) = app_handle.state::<db::DbState>().0.lock() {
                let log_preferences = extra_commands::read_log_preferences_from_conn(&conn);
                extra_commands::apply_log_preferences(&log_preferences);
            }
            provider_proxy::initialize_local_provider_proxy(&app_handle)
                .map_err(std::io::Error::other)?;
            utils::append_runtime_log("info", "app", "CCHub initialized");
            let initial_window_preferences = load_window_preferences(&app_handle);
            #[cfg(target_os = "linux")]
            {
                if let Err(error) = app.deep_link().register_all() {
                    utils::append_runtime_log(
                        "warn",
                        "deeplink",
                        &format!("Failed to register deep link schemes on Linux: {error}"),
                    );
                }
            }
            #[cfg(all(debug_assertions, windows))]
            {
                if let Err(error) = app.deep_link().register_all() {
                    utils::append_runtime_log(
                        "warn",
                        "deeplink",
                        &format!(
                            "Failed to register deep link schemes in Windows debug mode: {error}"
                        ),
                    );
                }
            }
            app.deep_link().on_open_url({
                let handle = app_handle.clone();
                move |event| {
                    for url in event.urls() {
                        if handle_deeplink_url(&handle, url.as_str(), true, "on-open-url") {
                            break;
                        }
                    }
                }
            });
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    if handle_deeplink_url(&app_handle, url.as_str(), true, "startup") {
                        break;
                    }
                }
            }
            webdav_sync::spawn_auto_sync_loop(app_handle.clone());
            s3_sync::spawn_auto_sync_loop(app_handle.clone());
            if let Some(tray) = app.tray_by_id("main") {
                {
                    let handle = app_handle.clone();
                    std::thread::spawn(move || {
                        if let Err(error) = tray::refresh_menu(&handle) {
                            utils::append_runtime_log(
                                "warn",
                                "tray",
                                &format!("Failed to build tray menu: {error}"),
                            );
                        }
                    });
                }
                let handle = app_handle.clone();
                tray.on_menu_event(move |_app, event| match event.id().as_ref() {
                    "show" => {
                        let _ = show_or_create_main_window(&handle, true);
                    }
                    "quit" => {
                        std::process::exit(0);
                    }
                    id if id.starts_with("profile:") => {
                        let profile_id = id.trim_start_matches("profile:");
                        if let Ok(conn) = handle.state::<db::DbState>().0.lock() {
                            let _ = extra_commands::apply_config_profile_from_conn(
                                &conn, profile_id, false,
                            );
                        }
                        let _ = tray::refresh_menu(&handle);
                    }
                    _ => {}
                });
                let handle2 = app_handle.clone();
                tray.on_tray_icon_event(move |_tray, event| {
                    if let TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } = event
                    {
                        let _ = show_or_create_main_window(&handle2, true);
                    }
                });
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = configure_main_window(&app_handle, &window);
                if !initial_window_preferences.launch_hidden {
                    let _ = window.show();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::mcp_commands::scan_mcp_servers,
            commands::mcp_commands::get_mcp_config,
            commands::mcp_commands::set_mcp_enabled,
            commands::mcp_commands::import_mcp_from_apps,
            commands::mcp_commands::get_mcp_servers,
            commands::mcp_commands::install_mcp_server,
            commands::mcp_commands::uninstall_mcp_server,
            commands::mcp_commands::update_mcp_server_config,
            commands::mcp_commands::toggle_mcp_server,
            commands::mcp_commands::check_mcp_server_health,
            commands::mcp_commands::check_all_mcp_health,
            commands::mcp_commands::sync_mcp_server_to_tool,
            commands::mcp_commands::unsync_mcp_server_from_tool,
            commands::mcp_commands::check_mcp_server_in_tools,
            commands::mcp_commands::check_runtime_dependencies,
            commands::mcp_commands::import_mcp_servers_from_file,
            skill_commands::scan_skills,
            skill_commands::get_skills,
            skill_commands::get_plugins,
            skill_commands::install_plugin,
            skill_commands::uninstall_plugin,
            skill_commands::read_skill_content,
            skill_commands::detect_tools,
            skill_commands::get_skill_folder_tree,
            skill_commands::check_path_exists,
            skill_commands::get_skill_categories,
            skill_commands::install_skill_file,
            skill_commands::uninstall_skill_file,
            skill_commands::get_skill_backups,
            skill_commands::restore_skill_backup,
            skill_commands::delete_skill_backup,
            skill_commands::copy_skill_between_tools,
            skill_commands::remove_synced_skill,
            skill_commands::write_skill_content,
            skill_commands::check_skill_updates,
            skill_commands::batch_update_skills,
            skill_commands::toggle_skill_file,
            skill_commands::delete_plugin_dir,
            skill_commands::get_skill_sync_method,
            skill_commands::set_skill_sync_method,
            skill_commands::import_skill_file,
            commands::skill_repository_commands::get_skill_repos,
            commands::skill_repository_commands::add_skill_repo,
            commands::skill_repository_commands::remove_skill_repo,
            commands::skill_repository_commands::discover_available_skills,
            commands::skill_repository_commands::install_skill,
            commands::skill_repository_commands::install_skill_for_app,
            commands::skill_repository_commands::uninstall_skill,
            commands::skill_repository_commands::toggle_skill_app,
            commands::skill_repository_commands::get_installed_skills,
            commands::skill_repository_commands::get_skills_for_app,
            commands::skill_repository_commands::update_skill,
            commands::skill_repository_commands::install_skills_from_zip,
            commands::skill_repository_commands::search_skills_sh,
            commands::hook_commands::scan_hooks,
            commands::hook_commands::get_hooks,
            commands::hook_commands::create_hook,
            commands::hook_commands::update_hook,
            commands::hook_commands::delete_hook,
            commands::hook_commands::save_hook_to_settings,
            commands::hook_commands::delete_hook_from_settings,
            commands::update_commands::check_updates,
            commands::update_commands::get_update_history,
            commands::update_commands::get_app_version,
            commands::claude_md_commands::scan_claude_md,
            commands::claude_md_commands::read_claude_md_content,
            commands::claude_md_commands::write_claude_md_content,
            commands::claude_md_commands::get_claude_md_templates,
            commands::claude_md_commands::get_prompt_presets,
            commands::claude_md_commands::save_prompt_preset,
            commands::claude_md_commands::delete_prompt_preset,
            commands::claude_md_commands::activate_prompt_preset,
            commands::claude_md_commands::create_new_claude_md,
            commands::claude_md_commands::create_instruction_doc_file,
            commands::claude_md_commands::delete_claude_md_file,
            commands::claude_md_commands::disable_claude_md_file,
            commands::claude_md_commands::enable_claude_md_file,
            commands::security_commands::run_security_audit,
            commands::security_commands::get_server_audit,
            commands::marketplace_commands::get_marketplace_entries,
            commands::marketplace_commands::search_marketplace,
            commands::marketplace_commands::install_from_marketplace,
            commands::marketplace_commands::get_skills_marketplace,
            commands::marketplace_commands::fetch_custom_skill_source,
            commands::marketplace_commands::install_skill_from_marketplace,
            commands::marketplace_commands::fetch_skills_from_repo,
            commands::marketplace_commands::get_skillhub_catalog,
            commands::marketplace_commands::search_skillhub_skills,
            commands::marketplace_commands::get_skillhub_skill_content,
            extra_commands::get_mcp_clients,
            extra_commands::create_mcp_client,
            extra_commands::update_mcp_client_access,
            extra_commands::delete_mcp_client,
            commands::mcp_commands::read_claude_mcp_config,
            commands::mcp_commands::upsert_claude_mcp_server,
            commands::mcp_commands::delete_claude_mcp_server,
            commands::mcp_commands::validate_mcp_command,
            extra_commands::get_activity_logs,
            extra_commands::get_activity_heatmap,
            extra_commands::get_workspaces,
            extra_commands::create_workspace,
            extra_commands::switch_workspace,
            extra_commands::update_workspace,
            extra_commands::delete_workspace,
            extra_commands::get_project_profiles,
            extra_commands::create_project_profile,
            extra_commands::update_project_profile,
            extra_commands::delete_project_profile,
            extra_commands::apply_project_profile,
            extra_commands::get_custom_paths,
            extra_commands::save_custom_path,
            extra_commands::delete_custom_path,
            commands::config_files_commands::get_config_roots,
            commands::config_files_commands::get_config_file_tree,
            commands::config_files_commands::read_config_file_content,
            commands::config_files_commands::write_config_file_content,
            extra_commands::read_codex_toml_structured,
            extra_commands::write_codex_toml_structured,
            extra_commands::sync_config_profiles,
            extra_commands::get_config_profiles,
            extra_commands::get_active_config_profile_ids,
            extra_commands::get_provider_config_fragments,
            extra_commands::save_provider_config_fragment,
            extra_commands::delete_provider_config_fragment,
            extra_commands::save_config_profile,
            extra_commands::save_shared_config_profiles,
            extra_commands::update_config_profile,
            extra_commands::apply_config_profile,
            extra_commands::delete_config_profile,
            extra_commands::delete_config_profile_group,
            extra_commands::reorder_config_profiles,
            extra_commands::get_common_config_snippet,
            extra_commands::set_common_config_snippet,
            extra_commands::ping_provider_endpoint,
            extra_commands::scan_provider_endpoints,
            extra_commands::probe_config_profile,
            extra_commands::stream_check_config_profile,
            extra_commands::stream_check_all_config_profiles,
            extra_commands::get_local_auth_status,
            commands::universal_provider_commands::get_universal_providers,
            commands::universal_provider_commands::get_universal_provider,
            commands::universal_provider_commands::upsert_universal_provider,
            commands::universal_provider_commands::delete_universal_provider,
            commands::universal_provider_commands::sync_universal_provider,
            extra_commands::refresh_tray_provider_menu,
            extra_commands::read_tool_config,
            extra_commands::write_tool_config,
            extra_commands::get_sessions,
            extra_commands::get_session_messages,
            extra_commands::get_session_detail,
            extra_commands::delete_session,
            extra_commands::delete_sessions,
            extra_commands::search_openclaw_daily_memory,
            extra_commands::read_openclaw_daily_memory_content,
            extra_commands::get_claude_permissions_level,
            extra_commands::set_claude_permissions_level,
            extra_commands::get_claude_auto_update,
            extra_commands::set_claude_auto_update,
            extra_commands::get_codex_settings,
            extra_commands::set_codex_setting,
            extra_commands::get_claude_model,
            extra_commands::set_claude_model,
            extra_commands::read_claude_config_toggles,
            extra_commands::write_claude_config_toggle,
            extra_commands::get_claude_tool_search,
            extra_commands::set_claude_tool_search,
            extra_commands::get_claude_hud_status,
            extra_commands::install_claude_hud,
            extra_commands::check_claude_hud_update,
            extra_commands::update_claude_hud,
            extra_commands::set_claude_statusline,
            extra_commands::set_claude_hud_config,
            extra_commands::get_hello2cc_status,
            extra_commands::install_hello2cc,
            extra_commands::uninstall_hello2cc,
            extra_commands::set_hello2cc_enabled,
            extra_commands::check_hello2cc_update,
            extra_commands::update_hello2cc,
            extra_commands::get_hello2cc_config,
            extra_commands::set_hello2cc_config,
            extra_commands::pick_folder,
            extra_commands::pick_file,
            extra_commands::set_proxy,
            extra_commands::get_proxy,
            extra_commands::test_proxy_url,
            extra_commands::scan_local_proxies,
            extra_commands::get_visible_apps,
            extra_commands::set_visible_apps,
            extra_commands::get_welcome_completed,
            extra_commands::set_welcome_completed,
            extra_commands::get_hermes_root_override,
            extra_commands::set_hermes_root_override,
            extra_commands::get_window_preferences,
            extra_commands::get_log_preferences,
            extra_commands::set_log_preferences,
            extra_commands::get_log_file_targets,
            extra_commands::get_updater_environment_state,
            extra_commands::set_window_preferences,
            extra_commands::get_terminal_preferences,
            extra_commands::set_preferred_terminal,
            extra_commands::open_in_preferred_terminal,
            extra_commands::resume_session_in_preferred_terminal,
            extra_commands::get_environment_conflicts,
            commands::provider_models::fetch_provider_models,
            commands::provider_models::fetch_provider_models_cached,
            commands::provider_models::fetch_provider_models_detailed,
            commands::provider_models::fetch_models_for_config,
            commands::models_dev_sync::get_models_dev_catalog,
            commands::models_dev_sync::sync_models_dev_pricing,
            commands::provider_compat::ensure_grokbuild_official_provider,
            commands::pi_compat::get_pi_current_state,
            commands::pi_compat::get_pi_session_discovery,
            commands::pi_compat::update_pi_provider_usage_script,
            commands::pi_prompt_compat::get_pi_prompt_file,
            commands::pi_prompt_compat::replace_pi_prompt_file,
            commands::pi_prompt_compat::delete_pi_prompt_file,
            commands::pi_prompt_compat::list_pi_prompt_templates,
            commands::pi_prompt_compat::upsert_pi_prompt_template,
            commands::pi_prompt_compat::delete_pi_prompt_template,
            commands::skill_compat::install_skill_unified,
            commands::skill_compat::scan_unmanaged_skills,
            commands::skill_compat::import_skills_from_apps,
            commands::skill_compat::migrate_skill_storage,
            commands::skill_compat::get_skill_storage_location,
            commands::skill_compat::set_skill_storage_location,
            commands::session_usage_compat::sync_session_usage,
            commands::session_usage_compat::rebuild_codex_usage,
            commands::provider_health_commands::get_provider_health,
            commands::provider_health_commands::get_provider_stats,
            commands::provider_health_commands::get_model_stats,
            commands::provider_models::get_cached_provider_models,
            provider_proxy::get_local_provider_proxy_settings,
            provider_proxy::get_local_provider_proxy_status,
            provider_proxy::set_local_provider_proxy_settings,
            extra_commands::save_backup_to_file,
            extra_commands::get_backup_preferences,
            extra_commands::set_backup_preferences,
            extra_commands::list_managed_backups,
            extra_commands::create_managed_backup,
            extra_commands::rename_managed_backup,
            extra_commands::delete_managed_backup,
            extra_commands::restore_managed_backup,
            extra_commands::run_scheduled_backup_if_needed,
            extra_commands::import_backup_from_file,
            extra_commands::remap_imported_project_root,
            extra_commands::get_pending_imported_project_roots,
            extra_commands::auto_remap_imported_project_roots,
            extra_commands::get_tool_environment_report,
            extra_commands::bootstrap_tool_environment,
            extra_commands::get_last_import_summary,
            extra_commands::run_full_rescan,
            extra_commands::repair_all_migration_issues,
            extra_commands::open_in_system,
            commands::usage_commands::get_proxy_usage_summary,
            commands::usage_commands::get_usage_summary,
            commands::usage_commands::get_usage_summary_by_app,
            commands::usage_commands::get_recent_proxy_request_logs,
            commands::usage_commands::get_request_detail,
            commands::usage_commands::search_proxy_request_logs,
            commands::usage_commands::get_proxy_usage_trend,
            commands::usage_commands::get_usage_trends,
            commands::usage_commands::get_usage_data_sources,
            commands::usage_analytics::get_usage_analytics,
            commands::usage_compat::get_balance,
            commands::usage_compat::get_coding_plan_quota,
            commands::usage_compat::queryProviderUsage,
            commands::stream_check_compat::get_stream_check_config,
            commands::stream_check_compat::save_stream_check_config,
            commands::startup_compat::import_default_config,
            commands::usage_commands::list_model_pricing,
            commands::usage_commands::save_model_pricing,
            commands::usage_commands::delete_model_pricing,
            commands::deeplink_commands::parse_deeplink,
            commands::deeplink_commands::merge_deeplink_request,
            commands::deeplink_commands::merge_deeplink_config,
            commands::deeplink_commands::import_from_deeplink,
            commands::deeplink_commands::import_from_deeplink_unified,
            commands::deeplink_commands::take_pending_deeplink_imports,
            commands::deeplink_commands::take_pending_deeplink_errors,
            commands::deeplink_commands::has_pending_deeplinks,
            commands::deeplink_commands::import_mcp_servers_from_deeplink,
            commands::failover_commands::get_failover_queue,
            commands::failover_commands::get_available_providers_for_failover,
            commands::failover_commands::add_to_failover_queue,
            commands::failover_commands::remove_from_failover_queue,
            commands::failover_commands::set_failover_queue,
            commands::failover_commands::get_auto_failover_enabled,
            commands::failover_commands::set_auto_failover_enabled,
            commands::claude_desktop_commands::get_claude_desktop_status,
            commands::claude_desktop_commands::get_claude_mcp_status,
            commands::claude_desktop_commands::get_claude_desktop_default_routes,
            commands::claude_desktop_commands::ensure_claude_desktop_official_provider,
            commands::claude_desktop_commands::import_claude_desktop_providers_from_claude,
            commands::claude_extension::get_claude_extension_integration,
            commands::claude_extension::set_claude_extension_integration,
            commands::claude_extension::get_claude_extension_status,
            commands::claude_extension::apply_claude_extension_config,
            commands::webdav_sync_commands::get_webdav_sync_settings,
            commands::webdav_sync_commands::set_webdav_sync_settings,
            commands::webdav_sync_commands::webdav_sync_save_settings,
            commands::webdav_sync_commands::webdav_test_connection,
            commands::webdav_sync_commands::webdav_sync_fetch_remote_info,
            commands::webdav_sync_commands::webdav_sync_upload,
            commands::webdav_sync_commands::webdav_sync_download,
            commands::s3_sync_commands::get_s3_sync_settings,
            commands::s3_sync_commands::set_s3_sync_settings,
            commands::s3_sync_commands::s3_sync_save_settings,
            commands::s3_sync_commands::s3_test_connection,
            commands::s3_sync_commands::s3_sync_fetch_remote_info,
            commands::s3_sync_commands::s3_sync_upload,
            commands::s3_sync_commands::s3_sync_download,
            commands::copilot_commands::copilot_start_device_flow,
            commands::copilot_commands::copilot_poll_for_account,
            commands::copilot_commands::copilot_list_accounts,
            commands::copilot_commands::copilot_remove_account,
            commands::copilot_commands::copilot_set_default_account,
            commands::copilot_commands::copilot_logout,
            commands::auth_commands::auth_start_login,
            commands::auth_commands::auth_poll_for_account,
            commands::auth_commands::auth_get_status,
            commands::auth_commands::auth_list_accounts,
            commands::auth_commands::auth_set_default_account,
            commands::auth_commands::auth_remove_account,
            commands::auth_commands::auth_logout,
            commands::copilot_commands::copilot_get_auth_status,
            commands::copilot_commands::copilot_get_token,
            commands::copilot_commands::copilot_get_usage,
            commands::copilot_commands::copilot_get_models,
            commands::copilot_commands::copilot_get_token_for_account,
            commands::copilot_commands::copilot_get_usage_for_account,
            commands::copilot_commands::copilot_get_models_for_account,
            commands::copilot_commands::copilot_is_authenticated,
            commands::copilot_commands::copilot_poll_for_auth,
            compat_commands::list_profiles,
            compat_commands::create_profile,
            compat_commands::update_profile,
            compat_commands::delete_profile,
            compat_commands::apply_profile,
            compat_commands::get_global_proxy_url,
            compat_commands::set_global_proxy_url,
            compat_commands::get_proxy_config,
            compat_commands::get_settings,
            compat_commands::save_settings,
            compat_commands::check_for_updates,
            compat_commands::check_env_conflicts,
            compat_commands::get_config_status,
            compat_commands::get_tool_versions,
            compat_commands::get_request_logs,
            compat_commands::stream_check_provider,
            compat_commands::stream_check_all_providers,
            compat_commands::get_proxy_status,
            compat_commands::get_upstream_proxy_status,
            compat_commands::is_proxy_running,
            compat_commands::start_proxy_server,
            compat_commands::stop_proxy_server,
            compat_commands::get_global_proxy_config,
            compat_commands::update_global_proxy_config,
            compat_commands::update_proxy_config,
            compat_commands::get_proxy_config_for_app,
            compat_commands::update_proxy_config_for_app,
            compat_commands::switch_provider,
            compat_commands::switch_proxy_provider,
            compat_commands::update_tray_menu,
            compat_commands::get_current_provider,
            compat_commands::list_sessions,
            compat_commands::get_model_pricing,
            compat_commands::check_app_update_available,
            compat_commands::get_provider_limits,
            compat_commands::check_provider_limits,
            compat_commands::get_claude_config_status,
            compat_commands::ensure_codex_official_provider,
            compat_commands::clear_current_profile,
            commands::prompt_library::get_prompts,
            commands::prompt_library::upsert_prompt,
            commands::prompt_library::delete_prompt,
            commands::prompt_library::enable_prompt,
            compat_commands::delete_mcp_server,
            compat_commands::delete_mcp_server_in_config,
            compat_commands::upsert_mcp_server,
            compat_commands::upsert_mcp_server_in_config,
            compat_commands::toggle_mcp_app,
            compat_commands::scan_openclaw_config_health,
            compat_commands::uninstall_skill_for_app,
            compat_commands::uninstall_skill_unified,
            compat_commands::get_log_config,
            compat_commands::set_log_config,
            compat_commands::update_circuit_breaker_config,
            compat_commands::update_model_pricing,
            compat_commands::update_model_pricing_batch,
            compat_commands::get_config_dir,
            compat_commands::get_claude_code_config_path,
            compat_commands::get_app_config_path,
            compat_commands::open_config_folder,
            compat_commands::open_app_config_folder,
            compat_commands::list_db_backups,
            compat_commands::create_db_backup,
            compat_commands::delete_db_backup,
            compat_commands::restore_db_backup,
            compat_commands::list_daily_memory_files,
            compat_commands::search_daily_memory_files,
            compat_commands::read_daily_memory_file,
            compat_commands::write_daily_memory_file,
            compat_commands::delete_daily_memory_file,
            compat_commands::get_hermes_live_provider_ids,
            compat_commands::get_hermes_live_provider,
            compat_commands::get_hermes_model_config,
            compat_commands::get_hermes_memory,
            compat_commands::set_hermes_memory,
            compat_commands::set_hermes_memory_enabled,
            compat_commands::get_claude_common_config_snippet,
            compat_commands::set_claude_common_config_snippet,
            compat_commands::get_claude_plugin_status,
            compat_commands::is_claude_plugin_applied,
            compat_commands::read_claude_plugin_config,
            compat_commands::apply_claude_plugin_config,
            compat_commands::apply_claude_onboarding_skip,
            compat_commands::clear_claude_onboarding_skip,
            compat_commands::get_proxy_takeover_status,
            compat_commands::is_live_takeover_active,
            compat_commands::stop_proxy_with_restore,
            compat_commands::pick_directory,
            compat_commands::open_file_dialog,
            compat_commands::open_zip_file_dialog,
            compat_commands::save_file_dialog,
            compat_commands::open_external,
            commands::workspace_compat::read_workspace_file,
            commands::workspace_compat::write_workspace_file,
            commands::workspace_compat::open_workspace_directory,
            commands::backup_compat::export_config_to_file,
            commands::backup_compat::import_config_from_file,
            commands::backup_compat::rename_db_backup,
            commands::lifecycle_compat::get_auto_launch_status,
            commands::lifecycle_compat::set_auto_launch,
            commands::lifecycle_compat::is_lightweight_mode,
            commands::lifecycle_compat::enter_lightweight_mode,
            commands::lifecycle_compat::exit_lightweight_mode,
            commands::lifecycle_compat::is_portable_mode,
            commands::lifecycle_compat::set_window_theme,
            commands::lifecycle_compat::get_app_config_dir_override,
            commands::lifecycle_compat::set_app_config_dir_override,
            commands::lifecycle_compat::set_proxy_takeover_for_app,
            commands::lifecycle_compat::get_default_cost_multiplier,
            commands::lifecycle_compat::set_default_cost_multiplier,
            commands::lifecycle_compat::get_pricing_model_source,
            commands::lifecycle_compat::set_pricing_model_source,
            commands::live_compat::read_live_provider_settings,
            commands::live_compat::sync_current_providers_live,
            commands::live_compat::update_providers_sort_order,
            commands::live_compat::update_endpoint_last_used,
            commands::live_compat::get_opencode_live_provider_ids,
            commands::live_compat::get_opencode_models,
            commands::live_compat::get_opencode_runtime_models,
            commands::extended_compat::get_models_dev_sync_config,
            commands::extended_compat::save_models_dev_sync_config,
            commands::extended_compat::delete_env_vars,
            commands::extended_compat::restore_env_backup,
            commands::extended_compat::test_api_endpoints,
            commands::extended_compat::testUsageScript,
            commands::extended_compat::launch_session_terminal,
            commands::extended_compat::open_provider_terminal,
            commands::extended_compat::import_hermes_providers_from_live,
            commands::extended_compat::import_openclaw_providers_from_live,
            commands::extended_compat::import_opencode_providers_from_live,
            commands::extended_compat::open_hermes_web_ui,
            commands::extended_compat::launch_hermes_dashboard,
            commands::extended_compat::extract_common_config_snippet,
            commands::extended_compat::update_toml_common_config_snippet,
            commands::prompt_library::get_current_prompt_file_content,
            commands::prompt_library::import_prompt_from_file,
            commands::extended_compat::get_init_error,
            commands::extended_compat::get_migration_result,
            commands::extended_compat::get_skills_migration_result,
            commands::codex_history_compat::has_codex_unify_history_backup,
            commands::codex_history_compat::restore_codex_unified_history,
            commands::codex_history_compat::migrate_codex_history,
            commands::extended_compat::restart_app,
            commands::extended_compat::install_update_and_restart,
            commands::extended_compat::copy_text_to_clipboard,
            commands::tool_lifecycle_compat::probe_tool_installations,
            commands::tool_lifecycle_compat::run_tool_lifecycle_action,
            commands::provider_compat::get_providers,
            commands::provider_compat::add_provider,
            commands::provider_compat::update_provider,
            commands::provider_compat::delete_provider,
            commands::provider_compat::get_custom_endpoints,
            commands::provider_compat::add_custom_endpoint,
            commands::provider_compat::remove_custom_endpoint,
            commands::oauth_commands::get_codex_cli_quota,
            commands::oauth_commands::get_codex_cli_models,
            commands::oauth_commands::get_codex_oauth_quota,
            commands::oauth_commands::get_codex_oauth_models,
            commands::oauth_commands::codex_oauth_start_device_flow,
            commands::oauth_commands::codex_oauth_poll_for_account,
            commands::oauth_commands::codex_oauth_list_accounts,
            commands::oauth_commands::codex_oauth_get_status,
            commands::oauth_commands::codex_oauth_remove_account,
            commands::oauth_commands::codex_oauth_set_default_account,
            commands::oauth_commands::codex_oauth_logout,
            commands::oauth_commands::get_claude_cli_quota,
            commands::oauth_commands::get_subscription_quota,
            commands::xai_oauth_commands::xai_oauth_start_device_flow,
            commands::xai_oauth_commands::xai_oauth_poll_for_account,
            commands::xai_oauth_commands::xai_oauth_list_accounts,
            commands::xai_oauth_commands::xai_oauth_get_status,
            commands::xai_oauth_commands::xai_oauth_remove_account,
            commands::xai_oauth_commands::xai_oauth_set_default_account,
            commands::xai_oauth_commands::xai_oauth_logout,
            commands::xai_oauth_commands::get_xai_oauth_models,
            commands::xai_oauth_commands::get_xai_oauth_quota,
            commands::omo_commands::omo_read_local_config,
            commands::omo_commands::omo_write_local_config,
            commands::omo_commands::disable_current_omo,
            commands::omo_commands::disable_current_omo_slim,
            commands::omo_commands::get_current_omo_provider_id,
            commands::omo_commands::get_current_omo_slim_provider_id,
            commands::omo_commands::read_omo_local_file,
            commands::omo_commands::read_omo_slim_local_file,
            commands::workflow_commands::scan_workflows,
            commands::workflow_commands::get_workflow_templates,
            commands::workflow_commands::install_workflow,
            commands::workflow_commands::read_workflow_content,
            commands::workflow_commands::write_workflow_content,
            commands::workflow_commands::delete_workflow,
            commands::workflow_commands::toggle_workflow,
            commands::workflow_commands::import_workflow_file,
            commands::autopilot_commands::get_autopilot_status,
            commands::autopilot_commands::pick_autopilot_file,
            commands::autopilot_commands::pick_autopilot_files,
            commands::autopilot_commands::start_autopilot,
            commands::autopilot_commands::stop_autopilot,
            commands::autopilot_commands::list_autopilot_logs,
            commands::autopilot_commands::delete_autopilot_log,
            commands::autopilot_commands::clear_autopilot_logs,
            commands::optimizer_commands::get_optimizer_config,
            commands::optimizer_commands::set_optimizer_config,
            commands::optimizer_commands::get_copilot_optimizer_config,
            commands::optimizer_commands::set_copilot_optimizer_config,
            commands::optimizer_commands::get_rectifier_config,
            commands::optimizer_commands::set_rectifier_config,
            commands::optimizer_commands::get_circuit_breaker_stats,
            commands::optimizer_commands::reset_circuit_breakers,
            commands::optimizer_commands::reset_circuit_breaker,
            commands::optimizer_commands::get_circuit_breaker_config,
            commands::hermes_commands::get_hermes_memory_limits,
            commands::hermes_commands::get_hermes_memory_content,
            commands::hermes_commands::save_hermes_memory_content,
            commands::hermes_commands::toggle_hermes_memory_enabled,
            commands::hermes_commands::list_hermes_providers,
            commands::hermes_commands::get_hermes_provider,
            commands::hermes_commands::save_hermes_provider,
            commands::hermes_commands::delete_hermes_provider,
            commands::hermes_commands::set_hermes_active_provider,
            commands::hermes_commands::get_hermes_active_provider,
            commands::openclaw_commands::get_openclaw_env,
            commands::openclaw_commands::set_openclaw_env,
            commands::openclaw_commands::get_openclaw_tools,
            commands::openclaw_commands::set_openclaw_tools,
            commands::openclaw_commands::get_openclaw_agents_defaults,
            commands::openclaw_commands::set_openclaw_agents_defaults,
            commands::openclaw_commands::scan_openclaw_health,
            commands::openclaw_commands::get_openclaw_status,
            commands::openclaw_commands::get_openclaw_model_catalog,
            commands::openclaw_commands::set_openclaw_model_catalog,
            commands::openclaw_commands::get_openclaw_default_model,
            commands::openclaw_commands::set_openclaw_default_model,
            commands::openclaw_commands::get_openclaw_live_provider_ids,
            commands::openclaw_commands::get_openclaw_live_provider,
            commands::openclaw_commands::remove_provider_from_live_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
