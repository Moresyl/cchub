use crate::db::models::Hook;
use crate::utils::atomic_write_string;
use rusqlite::Connection;
use std::collections::HashSet;

/// Read hooks from a settings.json file at the given path
fn read_hooks_from_file(path: &std::path::Path, scope: &str, project_path: Option<&str>) -> Vec<Hook> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let settings: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut hooks = Vec::new();

    if let Some(hooks_obj) = settings.get("hooks") {
        if let Some(obj) = hooks_obj.as_object() {
            for (event, hook_configs) in obj {
                if let Some(arr) = hook_configs.as_array() {
                    for (i, hook_config) in arr.iter().enumerate() {
                        let matcher = hook_config.get("matcher")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let command = hook_config.get("command")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let timeout = hook_config.get("timeout")
                            .and_then(|v| v.as_u64());

                        if command.is_empty() {
                            continue;
                        }

                        hooks.push(Hook {
                            id: format!(
                                "{}::{}::{}::{}",
                                scope,
                                project_path.unwrap_or("global"),
                                event,
                                i
                            ),
                            event: event.clone(),
                            matcher,
                            command,
                            scope: scope.to_string(),
                            project_path: project_path.map(String::from),
                            source_event: Some(event.clone()),
                            source_index: Some(i),
                            enabled: true,
                            timeout,
                        });
                    }
                }
            }
        }
    }

    hooks
}

fn discover_project_roots(conn: &Connection) -> Vec<String> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    let mut push_root = |path: String| {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return;
        }
        let key = trimmed.replace('\\', "/");
        if seen.insert(key) {
            roots.push(trimmed.to_string());
        }
    };

    if let Ok(mut stmt) = conn.prepare("SELECT base_path FROM workspaces WHERE base_path IS NOT NULL AND trim(base_path) != ''") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                push_root(row);
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare("SELECT project_path FROM hooks WHERE project_path IS NOT NULL AND trim(project_path) != ''") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                push_root(row);
            }
        }
    }

    let known_roots: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'known_project_roots'",
            [],
            |row| row.get(0),
        )
        .ok();
    if let Some(raw) = known_roots {
        if let Ok(paths) = serde_json::from_str::<Vec<String>>(&raw) {
            for path in paths {
                push_root(path);
            }
        }
    }

    roots
}

/// Read hooks from global and project ~/.claude/settings.json files.
pub fn read_hooks_from_settings(conn: &Connection) -> Vec<Hook> {
    let mut hooks = Vec::new();

    let path = match dirs::home_dir() {
        Some(h) => h.join(".claude").join("settings.json"),
        None => return hooks,
    };

    if path.exists() {
        hooks.extend(read_hooks_from_file(&path, "global", None));
    }

    for project_root in discover_project_roots(conn) {
        let project_settings = std::path::PathBuf::from(&project_root)
            .join(".claude")
            .join("settings.json");
        if project_settings.exists() {
            hooks.extend(read_hooks_from_file(
                &project_settings,
                "project",
                Some(&project_root),
            ));
        }
    }

    hooks.sort_by(|left, right| {
        left.scope
            .cmp(&right.scope)
            .then(left.project_path.cmp(&right.project_path))
            .then(left.event.cmp(&right.event))
            .then(left.source_index.cmp(&right.source_index))
    });
    hooks
}

/// Get the settings.json path for a given scope
fn get_settings_path(scope: &str, project_path: Option<&str>) -> Option<std::path::PathBuf> {
    match scope {
        "project" => {
            project_path.map(|p| std::path::PathBuf::from(p).join(".claude").join("settings.json"))
        }
        _ => dirs::home_dir().map(|h| h.join(".claude").join("settings.json")),
    }
}

/// Save a hook to the appropriate settings.json
pub fn save_hook_to_settings(
    event: &str,
    matcher: Option<&str>,
    command: &str,
    timeout: Option<u64>,
    scope: &str,
    project_path: Option<&str>,
    edit_index: Option<usize>,
) -> Result<(), String> {
    let path = get_settings_path(scope, project_path)
        .ok_or_else(|| "Cannot determine settings path".to_string())?;

    // Read existing settings or create empty object
    let mut settings: serde_json::Value = if path.exists() {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Ensure hooks object exists
    if settings.get("hooks").is_none() {
        settings["hooks"] = serde_json::json!({});
    }

    // Build the hook config object
    let mut hook_config = serde_json::json!({ "command": command });
    if let Some(m) = matcher {
        if !m.is_empty() {
            hook_config["matcher"] = serde_json::json!(m);
        }
    }
    if let Some(t) = timeout {
        hook_config["timeout"] = serde_json::json!(t);
    }

    // Get or create the event array
    let hooks_obj = settings["hooks"].as_object_mut()
        .ok_or_else(|| "hooks is not an object".to_string())?;

    if let Some(index) = edit_index {
        // Update existing hook at index
        if let Some(arr) = hooks_obj.get_mut(event).and_then(|v| v.as_array_mut()) {
            if index < arr.len() {
                arr[index] = hook_config;
            } else {
                return Err(format!("Hook index {} out of range", index));
            }
        } else {
            return Err(format!("Event '{}' not found", event));
        }
    } else {
        // Add new hook
        if let Some(arr) = hooks_obj.get_mut(event).and_then(|v| v.as_array_mut()) {
            arr.push(hook_config);
        } else {
            hooks_obj.insert(event.to_string(), serde_json::json!([hook_config]));
        }
    }

    // Write back
    let output = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    atomic_write_string(&path, &output).map_err(|e| e.to_string())?;
    Ok(())
}

/// Delete a hook from settings.json by event name and index
pub fn delete_hook_from_settings(
    event: &str,
    index: usize,
    scope: &str,
    project_path: Option<&str>,
) -> Result<(), String> {
    let path = get_settings_path(scope, project_path)
        .ok_or_else(|| "Cannot determine settings path".to_string())?;

    if !path.exists() {
        return Err("Settings file not found".to_string());
    }

    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let hooks_obj = settings.get_mut("hooks")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| "No hooks object in settings".to_string())?;

    let arr = hooks_obj.get_mut(event)
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| format!("Event '{}' not found", event))?;

    if index >= arr.len() {
        return Err(format!("Hook index {} out of range (total: {})", index, arr.len()));
    }

    arr.remove(index);

    // Remove the event key if array is now empty
    if arr.is_empty() {
        hooks_obj.remove(event);
    }

    // Remove hooks key entirely if empty
    if hooks_obj.is_empty() {
        settings.as_object_mut().unwrap().remove("hooks");
    }

    let output = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    atomic_write_string(&path, &output).map_err(|e| e.to_string())?;
    Ok(())
}
