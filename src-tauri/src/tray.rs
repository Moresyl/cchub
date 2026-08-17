use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    AppHandle, Manager,
};

use crate::{commands::extra_commands, db};

fn tool_label(tool_id: &str) -> &'static str {
    match tool_id {
        "claude" => "Claude",
        "codex" => "Codex",
        "gemini" => "Gemini",
        "opencode" => "OpenCode",
        "openclaw" => "OpenClaw",
        "hermes" => "Hermes",
        "pi" => "Pi",
        _ => "Provider",
    }
}

pub(crate) fn refresh_menu(app_handle: &AppHandle) -> Result<(), tauri::Error> {
    let show = MenuItemBuilder::with_id("show", "显示 CCHub").build(app_handle)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app_handle)?;
    let (profiles, active_ids) = {
        let db = app_handle.state::<db::DbState>();
        let result = match db.0.lock() {
            Ok(conn) => (
                extra_commands::read_config_profiles_for_tray(&conn).unwrap_or_default(),
                extra_commands::read_active_config_profile_ids_for_tray(&conn).unwrap_or_default(),
            ),
            Err(_) => (Vec::new(), Vec::new()),
        };
        result
    };
    let active_ids = active_ids
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let mut menu_builder = MenuBuilder::new(app_handle).item(&show);
    menu_builder = menu_builder.separator();
    let tool_order = [
        "claude", "codex", "gemini", "opencode", "openclaw", "hermes", "pi",
    ];
    for tool_id in tool_order {
        let tool_profiles = profiles
            .iter()
            .filter(|profile| profile.tool_id == tool_id)
            .collect::<Vec<_>>();
        if tool_profiles.is_empty() {
            continue;
        }
        let mut submenu = SubmenuBuilder::with_id(
            app_handle,
            format!("tool-group:{tool_id}"),
            tool_label(tool_id),
        );
        for profile in tool_profiles {
            let label = if active_ids.contains(&profile.id) {
                format!("• {}", profile.name)
            } else {
                profile.name.clone()
            };
            let menu_item = MenuItemBuilder::with_id(format!("profile:{}", profile.id), label)
                .build(app_handle)?;
            submenu = submenu.item(&menu_item);
        }
        menu_builder = menu_builder.item(&submenu.build()?);
    }
    let menu = menu_builder.item(&quit).build()?;
    if let Some(tray) = app_handle.tray_by_id("main") {
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}
