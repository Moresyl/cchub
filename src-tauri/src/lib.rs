mod claude_md;
mod commands;
mod copilot_auth;
mod db;
mod deeplink;
mod error;
mod hooks;
mod mcp;
mod omo;
mod provider_proxy;
mod provider_proxy_transform;
mod security;
mod skills;
mod updater;
mod utils;
mod webdav_sync;
mod workflows;

use commands::claude_md_commands;
use commands::autopilot_commands;
use commands::config_files_commands;
use commands::copilot_commands;
use commands::deeplink_commands;
use commands::extra_commands;
use commands::hook_commands;
use commands::marketplace_commands;
use commands::mcp_commands;
use commands::omo_commands;
use commands::security_commands;
use commands::skill_commands;
use commands::update_commands;
use commands::usage_commands;
use commands::webdav_sync_commands;
use commands::workflow_commands;

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconEvent,
    AppHandle, Emitter, Manager,
};
use tauri_plugin_deep_link::DeepLinkExt;

const WINDOW_ICON: Image<'_> = tauri::include_image!("./icons/icon.png");

fn redact_url_for_log(url_str: &str) -> String {
    match url::Url::parse(url_str) {
        Ok(url) => {
            let mut output = format!("{}://", url.scheme());
            if let Some(host) = url.host_str() {
                output.push_str(host);
            }
            output.push_str(url.path());

            let mut keys: Vec<String> = url.query_pairs().map(|(key, _)| key.to_string()).collect();
            keys.sort();
            keys.dedup();
            if !keys.is_empty() {
                output.push_str("?[keys:");
                output.push_str(&keys.join(","));
                output.push(']');
            }

            output
        }
        Err(_) => {
            let base = url_str.split('#').next().unwrap_or(url_str);
            match base.split_once('?') {
                Some((prefix, _)) => format!("{prefix}?[redacted]"),
                None => base.to_string(),
            }
        }
    }
}

fn focus_main_window(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn handle_deeplink_url(
    app_handle: &AppHandle,
    url_str: &str,
    focus_window: bool,
    source: &str,
) -> bool {
    if !url_str.starts_with("cchub://") && !url_str.starts_with("ccswitch://") {
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

fn tool_label_for_tray(tool_id: &str) -> &'static str {
    match tool_id {
        "claude" => "Claude",
        "codex" => "Codex",
        "gemini" => "Gemini",
        "opencode" => "OpenCode",
        "openclaw" => "OpenClaw",
        _ => "Provider",
    }
}

fn load_window_preferences(app_handle: &tauri::AppHandle) -> extra_commands::WindowPreferences {
    let db = app_handle.state::<db::DbState>();
    let preferences = match db.0.lock() {
        Ok(conn) => extra_commands::read_window_preferences_from_conn(&conn),
        Err(_) => extra_commands::WindowPreferences::default(),
    };
    preferences
}

pub(crate) fn refresh_tray_menu(app_handle: &AppHandle) -> Result<(), tauri::Error> {
    let show = MenuItemBuilder::with_id("show", "显示 CCHub").build(app_handle)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app_handle)?;

    let (profiles, active_ids) = {
        let db = app_handle.state::<db::DbState>();
        let tray_data = match db.0.lock() {
            Ok(conn) => (
                extra_commands::read_config_profiles_for_tray(&conn).unwrap_or_default(),
                extra_commands::read_active_config_profile_ids_for_tray(&conn).unwrap_or_default(),
            ),
            Err(_) => (Vec::new(), Vec::new()),
        };
        tray_data
    };

    let active_ids = active_ids
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let mut menu_builder = MenuBuilder::new(app_handle).item(&show);

    if !profiles.is_empty() {
        menu_builder = menu_builder.separator();
        let mut previous_tool_id: Option<String> = None;

        for profile in profiles {
            if previous_tool_id.as_deref() != Some(profile.tool_id.as_str()) {
                previous_tool_id = Some(profile.tool_id.clone());
            }

            let label = if active_ids.contains(&profile.id) {
                format!(
                    "• {}: {}",
                    tool_label_for_tray(&profile.tool_id),
                    profile.name
                )
            } else {
                format!(
                    "{}: {}",
                    tool_label_for_tray(&profile.tool_id),
                    profile.name
                )
            };
            let menu_item = MenuItemBuilder::with_id(format!("profile:{}", profile.id), label)
                .build(app_handle)?;
            menu_builder = menu_builder.item(&menu_item);
        }

        menu_builder = menu_builder.separator();
    } else {
        menu_builder = menu_builder.separator();
    }

    let menu = menu_builder.item(&quit).build()?;
    if let Some(tray) = app_handle.tray_by_id("main") {
        tray.set_menu(Some(menu))?;
    }

    Ok(())
}

pub fn run() {
    utils::install_panic_hook();

    let mut builder = tauri::Builder::default()
        .manage(deeplink::DeepLinkState::default())
        .manage(autopilot_commands::AutopilotRuntime::default());

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
            copilot_auth::init_copilot_auth_state(&app_handle);
            provider_proxy::init_local_provider_proxy_runtime(&app_handle);
            if let Ok(conn) = app_handle.state::<db::DbState>().0.lock() {
                let log_preferences = extra_commands::read_log_preferences_from_conn(&conn);
                extra_commands::apply_log_preferences(&log_preferences);
            }
            provider_proxy::initialize_local_provider_proxy(&app_handle)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
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

            // Attach menu to tray icon (created by tauri.conf.json trayIcon config)
            if let Some(tray) = app.tray_by_id("main") {
                refresh_tray_menu(&app_handle)?;
                let handle = app_handle.clone();
                tray.on_menu_event(move |_app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = handle.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        std::process::exit(0);
                    }
                    id if id.starts_with("profile:") => {
                        let profile_id = id.trim_start_matches("profile:");
                        if let Ok(conn) = handle.state::<db::DbState>().0.lock() {
                            let _ =
                                extra_commands::apply_config_profile_from_conn(&conn, profile_id);
                        }
                        let _ = refresh_tray_menu(&handle);
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
                        if let Some(w) = handle2.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                });
            }

            // Handle window close → hide to tray instead of quit
            let handle3 = app_handle.clone();
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_icon(WINDOW_ICON.clone());
                // 窗口在 tauri.conf.json 中默认 visible=false,等 setup() 完成后再显示,
                // 避免用户看到 WebView2 冷启动阶段的白屏。launch_hidden 用户保持隐藏。
                if !initial_window_preferences.launch_hidden {
                    let _ = window.show();
                }
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let preferences = load_window_preferences(&handle3);
                        api.prevent_close();
                        if preferences.close_to_tray {
                            if let Some(w) = handle3.get_webview_window("main") {
                                let _ = w.hide();
                            }
                        } else {
                            handle3.exit(0);
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // MCP commands
            mcp_commands::scan_mcp_servers,
            mcp_commands::get_mcp_servers,
            mcp_commands::install_mcp_server,
            mcp_commands::uninstall_mcp_server,
            mcp_commands::update_mcp_server_config,
            mcp_commands::toggle_mcp_server,
            mcp_commands::check_mcp_server_health,
            mcp_commands::check_all_mcp_health,
            mcp_commands::sync_mcp_server_to_tool,
            mcp_commands::unsync_mcp_server_from_tool,
            mcp_commands::check_mcp_server_in_tools,
            mcp_commands::check_runtime_dependencies,
            mcp_commands::import_mcp_servers_from_file,
            // Skill commands
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
            skill_commands::toggle_skill_file,
            skill_commands::delete_plugin_dir,
            skill_commands::get_skill_sync_method,
            skill_commands::set_skill_sync_method,
            skill_commands::import_skill_file,
            // Hook commands
            hook_commands::scan_hooks,
            hook_commands::get_hooks,
            hook_commands::create_hook,
            hook_commands::update_hook,
            hook_commands::delete_hook,
            hook_commands::save_hook_to_settings,
            hook_commands::delete_hook_from_settings,
            // Update commands
            update_commands::check_updates,
            update_commands::get_update_history,
            update_commands::get_app_version,
            // CLAUDE.md commands
            claude_md_commands::scan_claude_md,
            claude_md_commands::read_claude_md_content,
            claude_md_commands::write_claude_md_content,
            claude_md_commands::get_claude_md_templates,
            claude_md_commands::get_prompt_presets,
            claude_md_commands::save_prompt_preset,
            claude_md_commands::delete_prompt_preset,
            claude_md_commands::activate_prompt_preset,
            claude_md_commands::create_new_claude_md,
            claude_md_commands::create_instruction_doc_file,
            claude_md_commands::delete_claude_md_file,
            claude_md_commands::disable_claude_md_file,
            claude_md_commands::enable_claude_md_file,
            // Security commands
            security_commands::run_security_audit,
            security_commands::get_server_audit,
            // Marketplace commands
            marketplace_commands::get_marketplace_entries,
            marketplace_commands::search_marketplace,
            marketplace_commands::install_from_marketplace,
            marketplace_commands::get_skills_marketplace,
            marketplace_commands::fetch_custom_skill_source,
            marketplace_commands::install_skill_from_marketplace,
            marketplace_commands::fetch_skills_from_repo,
            marketplace_commands::get_skillhub_catalog,
            marketplace_commands::search_skillhub_skills,
            marketplace_commands::get_skillhub_skill_content,
            // Extra commands (clients, logs, workspaces)
            extra_commands::get_mcp_clients,
            extra_commands::create_mcp_client,
            extra_commands::update_mcp_client_access,
            extra_commands::delete_mcp_client,
            extra_commands::get_activity_logs,
            extra_commands::get_activity_heatmap,
            extra_commands::get_workspaces,
            extra_commands::create_workspace,
            extra_commands::switch_workspace,
            extra_commands::update_workspace,
            extra_commands::delete_workspace,
            // Custom paths
            extra_commands::get_custom_paths,
            extra_commands::save_custom_path,
            extra_commands::delete_custom_path,
            // Config file editor
            config_files_commands::get_config_roots,
            config_files_commands::get_config_file_tree,
            config_files_commands::read_config_file_content,
            config_files_commands::write_config_file_content,
            extra_commands::read_codex_toml_structured,
            extra_commands::write_codex_toml_structured,
            // Config profiles
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
            extra_commands::probe_config_profile,
            extra_commands::stream_check_config_profile,
            extra_commands::refresh_tray_provider_menu,
            extra_commands::read_tool_config,
            extra_commands::write_tool_config,
            extra_commands::get_sessions,
            extra_commands::get_session_detail,
            extra_commands::delete_session,
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
            extra_commands::get_visible_apps,
            extra_commands::set_visible_apps,
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
            usage_commands::get_proxy_usage_summary,
            usage_commands::get_recent_proxy_request_logs,
            usage_commands::search_proxy_request_logs,
            usage_commands::get_proxy_usage_trend,
            usage_commands::list_model_pricing,
            usage_commands::save_model_pricing,
            usage_commands::delete_model_pricing,
            deeplink_commands::parse_deeplink,
            deeplink_commands::merge_deeplink_request,
            deeplink_commands::take_pending_deeplink_imports,
            deeplink_commands::take_pending_deeplink_errors,
            deeplink_commands::import_mcp_servers_from_deeplink,
            webdav_sync_commands::get_webdav_sync_settings,
            webdav_sync_commands::set_webdav_sync_settings,
            webdav_sync_commands::webdav_test_connection,
            webdav_sync_commands::webdav_sync_fetch_remote_info,
            webdav_sync_commands::webdav_sync_upload,
            webdav_sync_commands::webdav_sync_download,
            copilot_commands::copilot_start_device_flow,
            copilot_commands::copilot_poll_for_account,
            copilot_commands::copilot_list_accounts,
            copilot_commands::copilot_remove_account,
            copilot_commands::copilot_set_default_account,
            copilot_commands::copilot_get_auth_status,
            copilot_commands::copilot_get_token,
            omo_commands::omo_read_local_config,
            omo_commands::omo_write_local_config,
            // Workflow commands
            workflow_commands::scan_workflows,
            workflow_commands::get_workflow_templates,
            workflow_commands::install_workflow,
            workflow_commands::read_workflow_content,
            workflow_commands::write_workflow_content,
            workflow_commands::delete_workflow,
            workflow_commands::toggle_workflow,
            workflow_commands::import_workflow_file,
            autopilot_commands::get_autopilot_status,
            autopilot_commands::pick_autopilot_file,
            autopilot_commands::start_autopilot,
            autopilot_commands::stop_autopilot,
            autopilot_commands::list_autopilot_logs,
            autopilot_commands::delete_autopilot_log,
            autopilot_commands::clear_autopilot_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
