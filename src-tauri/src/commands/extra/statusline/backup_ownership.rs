use rusqlite::{params, Connection};

// Only shared configuration libraries travel through cloud settings. Unknown
// settings stay local by default, so newly added device credentials/path state
// cannot become portable merely because a developer forgot this list.
const PORTABLE_SETTINGS: &[&str] = &[
    "provider_config_fragments",
    "common_config_snippets",
    "universal_providers",
    "skill_repositories",
    "proxy_optimizer_config",
    "rectifier_config",
    "stream_check_config",
];

pub(super) fn preserve_device_state(
    live: &Connection,
    prepared: &Connection,
) -> Result<usize, String> {
    let mut preserved_rows = 0;
    let mut stmt = live
        .prepare("SELECT key, value FROM app_settings")
        .map_err(|_| "无法读取本机设置")?;
    let settings = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|_| "无法读取本机设置")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "本机设置记录无效")?;
    let mut stmt = prepared
        .prepare("SELECT key FROM app_settings")
        .map_err(|_| "无法读取备份设置")?;
    let imported = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|_| "无法读取备份设置")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "备份设置记录无效")?;
    for key in imported
        .into_iter()
        .filter(|key| !PORTABLE_SETTINGS.contains(&key.as_str()))
    {
        prepared
            .execute("DELETE FROM app_settings WHERE key=?1", [key])
            .map_err(|_| "无法隔离备份中的设备设置")?;
    }
    for (key, value) in settings
        .into_iter()
        .filter(|(key, _)| !PORTABLE_SETTINGS.contains(&key.as_str()))
    {
        prepared
            .execute(
                "INSERT OR REPLACE INTO app_settings (key,value) VALUES (?1,?2)",
                params![key, value],
            )
            .map_err(|_| "无法保留本机设置")?;
        preserved_rows += 1;
    }
    prepared
        .execute("DELETE FROM custom_paths", [])
        .map_err(|_| "无法准备本机工具目录")?;
    let mut stmt = live
        .prepare("SELECT tool_id, config_dir, mcp_config_path, skills_dir FROM custom_paths")
        .map_err(|_| "无法读取本机工具目录")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|_| "无法读取本机工具目录")?;
    for row in rows {
        let (tool, config, mcp, skills) = row.map_err(|_| "本机工具目录记录无效")?;
        prepared.execute("INSERT INTO custom_paths (tool_id, config_dir, mcp_config_path, skills_dir) VALUES (?1,?2,?3,?4)", params![tool,config,mcp,skills])
            .map_err(|_| "无法保留本机工具目录")?;
        preserved_rows += 1;
    }
    // Imported workspaces appear as unconfigured until the user chooses their
    // local directories. Existing local workspaces retain their exact binding.
    prepared
        .execute("UPDATE workspaces SET base_path=NULL, is_active=0", [])
        .map_err(|_| "无法准备项目路径迁移")?;
    let mut stmt = live
        .prepare("SELECT id,name,description,base_path,is_active,created_at FROM workspaces")
        .map_err(|_| "无法读取本机工作区")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|_| "无法读取本机工作区")?;
    for row in rows {
        let (id, name, description, path, active, created) =
            row.map_err(|_| "本机工作区记录无效")?;
        prepared.execute("INSERT OR REPLACE INTO workspaces (id,name,description,base_path,is_active,created_at) VALUES (?1,?2,?3,?4,?5,?6)", params![id,name,description,path,active,created])
            .map_err(|_| "无法保留本机工作区")?;
        preserved_rows += 1;
    }
    Ok(preserved_rows)
}

#[cfg(test)]
mod tests;
