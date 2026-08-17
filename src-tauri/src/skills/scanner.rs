use super::tools::detect_tools_for_conn;
use super::updater;
use crate::commands::extra_commands::configured_skill_storage_dir;
use crate::db::models::{Plugin, Skill};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct InstalledPluginRoot {
    id: String,
    path: PathBuf,
    version: Option<String>,
    installed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FolderNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<FolderNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryCounts {
    pub skills: u32,
    pub prompts: u32,
    pub commands: u32,
    pub plugins: u32,
    pub total: u32,
}

/// Build a folder tree from a base directory
pub fn get_folder_tree(base_dir: &str) -> Result<FolderNode, String> {
    let path = PathBuf::from(base_dir);
    if !path.exists() {
        return Err(format!("Directory does not exist: {}", base_dir));
    }
    Ok(build_tree(&path, 3, 0))
}

fn build_tree(path: &PathBuf, max_depth: usize, depth: usize) -> FolderNode {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut node = FolderNode {
        name,
        path: path.to_string_lossy().to_string(),
        is_dir: path.is_dir(),
        children: Vec::new(),
    };

    if path.is_dir() && depth < max_depth {
        if let Ok(entries) = std::fs::read_dir(path) {
            let mut children: Vec<FolderNode> = entries
                .flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    !n.starts_with('.') && n != "node_modules"
                })
                .map(|e| build_tree(&e.path(), max_depth, depth + 1))
                .collect();
            // Directories first, then files, alphabetical within each
            children.sort_by(|a, b| {
                b.is_dir
                    .cmp(&a.is_dir)
                    .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            node.children = children;
        }
    }

    node
}

/// Categorize skills by examining file content/path patterns
pub fn get_category_counts(skills: &[Skill]) -> CategoryCounts {
    let mut counts = CategoryCounts {
        skills: 0,
        prompts: 0,
        commands: 0,
        plugins: 0,
        total: skills.len() as u32,
    };

    for skill in skills {
        let category = categorize_skill(skill);
        match category.as_str() {
            "prompt" => counts.prompts += 1,
            "command" => counts.commands += 1,
            "plugin" => counts.plugins += 1,
            _ => counts.skills += 1,
        }
    }

    counts
}

/// Determine skill category from metadata
pub fn categorize_skill(skill: &Skill) -> String {
    // If it belongs to a plugin, mark as plugin-skill
    if skill.plugin_id.is_some() {
        return "plugin".to_string();
    }

    let name_lower = skill.name.to_lowercase();
    let desc_lower = skill.description.as_deref().unwrap_or("").to_lowercase();

    // Check for prompt patterns
    if name_lower.contains("prompt")
        || desc_lower.contains("prompt")
        || desc_lower.contains("template")
    {
        return "prompt".to_string();
    }

    // Check for command patterns
    if skill.trigger_command.is_some()
        && (name_lower.contains("command")
            || desc_lower.contains("command")
            || desc_lower.contains("slash"))
    {
        return "command".to_string();
    }

    "skill".to_string()
}

/// Check if a path exists
pub fn check_path_exists(path: &str) -> bool {
    PathBuf::from(path).exists()
}

/// Get the Claude Code plugins directory
pub fn get_plugins_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("plugins"))
}

/// Get the Claude Code skills directory (user-level)
pub fn get_skills_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("skills"))
}

/// Scan locally installed plugins from ~/.claude/plugins/
pub fn scan_local_plugins() -> Vec<Plugin> {
    discover_installed_plugin_roots()
        .into_iter()
        .map(|root| {
            let (description, package_version, source_url) = read_plugin_metadata(&root.path);
            let name = root
                .id
                .rsplit_once('@')
                .map(|(name, _)| name)
                .filter(|name| !name.is_empty())
                .unwrap_or(&root.id)
                .to_string();
            Plugin {
                id: root.id,
                name,
                description,
                source_url,
                version: root.version.or(package_version),
                installed_at: root
                    .installed_at
                    .or_else(|| get_dir_created_time(&root.path)),
                updated_at: get_dir_modified_time(&root.path),
            }
        })
        .collect()
}

fn discover_installed_plugin_roots() -> Vec<InstalledPluginRoot> {
    let Some(plugins_dir) = get_plugins_dir() else {
        return Vec::new();
    };
    let mut roots = Vec::new();
    let mut seen_paths = HashSet::new();

    let manifest_path = plugins_dir.join("installed_plugins.json");
    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(entries) = value.get("plugins").and_then(serde_json::Value::as_object) {
                for (id, records) in entries {
                    let Some(record) = records.as_array().and_then(|items| {
                        items
                            .iter()
                            .rev()
                            .find(|item| item.get("installPath").is_some())
                    }) else {
                        continue;
                    };
                    let Some(path) = record
                        .get("installPath")
                        .and_then(serde_json::Value::as_str)
                        .map(PathBuf::from)
                        .filter(|path| path.is_dir())
                    else {
                        continue;
                    };
                    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                    if !seen_paths.insert(canonical) {
                        continue;
                    }
                    roots.push(InstalledPluginRoot {
                        id: id.clone(),
                        path,
                        version: record
                            .get("version")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string),
                        installed_at: record
                            .get("installedAt")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string),
                    });
                }
            }
        }
    }

    if let Ok(entries) = std::fs::read_dir(&plugins_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() || !is_direct_plugin_root(&path) {
                continue;
            }
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !seen_paths.insert(canonical) {
                continue;
            }
            let id = entry.file_name().to_string_lossy().to_string();
            roots.push(InstalledPluginRoot {
                id,
                path,
                version: None,
                installed_at: None,
            });
        }
    }

    roots
}

fn is_direct_plugin_root(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if matches!(name, "cache" | "marketplaces" | "repos" | "downloads") {
        return false;
    }
    path.join("package.json").is_file()
        || path.join(".claude-plugin").join("plugin.json").is_file()
        || path.join("skills").is_dir()
        || path.join("commands").is_dir()
}

pub fn scan_local_skills_for_conn(conn: &Connection) -> Vec<Skill> {
    let plan = prepare_local_skill_scan(conn);
    scan_local_skills_from_plan(&plan)
}

#[derive(Debug, Clone)]
struct SkillScanRoot {
    path: PathBuf,
    plugin_id: Option<String>,
    tool_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SkillScanPlan {
    roots: Vec<SkillScanRoot>,
    metadata_map: HashMap<String, updater::SkillSourceMetadata>,
}

/// Snapshot database-backed scan configuration while the SQLite lock is held.
/// The expensive directory walk and hashing can then run after releasing it.
pub(crate) fn prepare_local_skill_scan(conn: &Connection) -> SkillScanPlan {
    let metadata_map = updater::load_skill_metadata_map(conn).unwrap_or_default();
    let detected_tools = detect_tools_for_conn(conn);
    let mut roots = Vec::new();
    let mut scanned_dirs = HashSet::new();

    // The Claude plugin manifest is the source of truth for cached installs.
    // Direct CCHub installs are included only when they contain plugin markers.
    for plugin in discover_installed_plugin_roots() {
        roots.push(SkillScanRoot {
            path: plugin.path,
            plugin_id: Some(plugin.id),
            tool_id: Some("claude".to_string()),
        });
    }

    // Scan standalone skills for every detected tool
    if let Some(shared_dir) = configured_skill_storage_dir(conn) {
        scanned_dirs.insert(shared_dir.clone());
        let shared_tool = detected_tools
            .iter()
            .find(|tool| tool.installed)
            .map(|tool| tool.id.clone())
            .unwrap_or_else(|| "claude".to_string());
        roots.push(SkillScanRoot {
            path: shared_dir,
            plugin_id: None,
            tool_id: Some(shared_tool),
        });
    }
    for tool in detected_tools.into_iter().filter(|tool| tool.installed) {
        let skills_dir = crate::commands::extra_commands::resolve_tool_skills_dir(conn, &tool.id)
            .ok()
            .unwrap_or_else(|| PathBuf::from(&tool.skills_dir));
        if !scanned_dirs.insert(skills_dir.clone()) {
            continue;
        }
        roots.push(SkillScanRoot {
            path: skills_dir,
            plugin_id: None,
            tool_id: Some(tool.id),
        });
    }

    SkillScanPlan {
        roots,
        metadata_map,
    }
}

pub(crate) fn scan_local_skills_from_plan(plan: &SkillScanPlan) -> Vec<Skill> {
    let mut skills = Vec::new();
    for root in &plan.roots {
        if root.path.exists() {
            scan_skills_in_dir(
                &root.path,
                &mut skills,
                root.plugin_id.as_deref(),
                root.tool_id.as_deref(),
                &plan.metadata_map,
            );
        }
    }

    skills
}

fn scan_skills_in_dir(
    dir: &PathBuf,
    skills: &mut Vec<Skill>,
    plugin_id: Option<&str>,
    tool_id: Option<&str>,
    metadata_map: &HashMap<String, updater::SkillSourceMetadata>,
) {
    let is_plugin_dir = plugin_id.is_some();
    let walker = walkdir(dir, is_plugin_dir);
    for skill_file in walker
        .into_iter()
        .filter(|path| is_skill_candidate(path, is_plugin_dir))
    {
        if let Some(skill) = parse_skill_file(&skill_file, plugin_id, tool_id, metadata_map) {
            skills.push(skill);
        }
    }
}

fn is_skill_candidate(path: &Path, is_plugin_dir: bool) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !(file_name.ends_with(".md") || file_name.ends_with(".md.disabled")) {
        return false;
    }
    if matches!(
        file_name.as_str(),
        "readme.md" | "readme.md.disabled" | "changelog.md" | "changelog.md.disabled"
    ) {
        return false;
    }
    if !is_plugin_dir || matches!(file_name.as_str(), "skill.md" | "skill.md.disabled") {
        return true;
    }
    path.ancestors().any(|ancestor| {
        matches!(
            ancestor.file_name().and_then(|name| name.to_str()),
            Some("skills" | "commands" | "agents")
        )
    })
}

fn walkdir(dir: &PathBuf, deep: bool) -> Vec<PathBuf> {
    let mut results = Vec::new();
    walk_recursive(dir, &mut results, if deep { 4 } else { 2 }, 0);
    results
}

fn walk_recursive(
    dir: &PathBuf,
    results: &mut Vec<PathBuf>,
    max_depth: usize,
    current_depth: usize,
) {
    if current_depth > max_depth {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let file_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");
                if file_name.ends_with(".md") || file_name.ends_with(".md.disabled") {
                    results.push(path);
                }
            } else if path.is_dir() {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !dir_name.starts_with('.') && dir_name != "node_modules" {
                    walk_recursive(&path, results, max_depth, current_depth + 1);
                }
            }
        }
    }
}

fn parse_skill_file(
    path: &PathBuf,
    plugin_id: Option<&str>,
    tool_id: Option<&str>,
    metadata_map: &HashMap<String, updater::SkillSourceMetadata>,
) -> Option<Skill> {
    let content = std::fs::read_to_string(path).ok()?;
    let raw_file_name = path.file_name()?.to_string_lossy();
    let file_name = raw_file_name
        .strip_suffix(".md.disabled")
        .or_else(|| raw_file_name.strip_suffix(".md"))
        .unwrap_or(&raw_file_name)
        .to_string();
    let path_key = path.to_string_lossy().to_string();
    let metadata = metadata_map.get(&path_key);
    let current_sha256 = Some(updater::sha256_hex(&content));

    // Extract frontmatter metadata
    let (name, description, trigger) = if content.starts_with("---") {
        parse_frontmatter(&content, &file_name)
    } else {
        (file_name.clone(), None, None)
    };

    Some(Skill {
        id: path_key,
        name,
        description,
        tool_id: tool_id.map(str::to_string),
        plugin_id: plugin_id.map(str::to_string),
        trigger_command: trigger,
        file_path: Some(path.to_string_lossy().to_string()),
        version: None,
        installed_at: get_file_created_time(path),
        source_url: metadata.and_then(|item| item.source_url.clone()),
        baseline_sha256: metadata.and_then(|item| item.baseline_sha256.clone()),
        latest_sha256: metadata.and_then(|item| item.latest_sha256.clone()),
        last_checked_at: metadata.and_then(|item| item.last_checked_at),
        current_sha256,
    })
}

fn parse_frontmatter(
    content: &str,
    default_name: &str,
) -> (String, Option<String>, Option<String>) {
    let mut name = default_name.to_string();
    let mut description = None;
    let mut trigger = None;

    if let Some(end) = content[3..].find("---") {
        let frontmatter = &content[3..3 + end];
        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("name:") {
                name = val.trim().trim_matches('"').trim_matches('\'').to_string();
            } else if let Some(val) = line.strip_prefix("description:") {
                description = Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
            } else if let Some(val) = line.strip_prefix("trigger:") {
                trigger = Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
            }
        }
    }

    (name, description, trigger)
}

fn read_plugin_metadata(path: &Path) -> (Option<String>, Option<String>, Option<String>) {
    let pkg_json = path.join("package.json");
    if pkg_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&pkg_json) {
            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                return (
                    pkg.get("description")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    pkg.get("version")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    pkg.get("repository")
                        .and_then(|v| v.get("url").or(Some(v)))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                );
            }
        }
    }
    (None, None, None)
}

fn get_dir_created_time(path: &PathBuf) -> Option<String> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.created().ok())
        .map(|t| {
            let dt: chrono::DateTime<chrono::Utc> = t.into();
            dt.to_rfc3339()
        })
}

fn get_dir_modified_time(path: &PathBuf) -> Option<String> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            let dt: chrono::DateTime<chrono::Utc> = t.into();
            dt.to_rfc3339()
        })
}

fn get_file_created_time(path: &PathBuf) -> Option<String> {
    get_dir_created_time(path)
}

#[cfg(test)]
mod tests {
    use super::{scan_local_skills_from_plan, SkillScanPlan, SkillScanRoot};
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn scans_from_a_database_free_plan() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cchub-skill-scan-{unique}"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("sample.md"),
            "---\nname: Fast Skill\ndescription: scan without database lock\n---\n",
        )
        .unwrap();

        let plan = SkillScanPlan {
            roots: vec![SkillScanRoot {
                path: root.clone(),
                plugin_id: None,
                tool_id: Some("codex".to_string()),
            }],
            metadata_map: HashMap::new(),
        };
        let skills = scan_local_skills_from_plan(&plan);

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "Fast Skill");
        assert_eq!(skills[0].tool_id.as_deref(), Some("codex"));
        assert!(skills[0].current_sha256.is_some());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn includes_disabled_skills_but_excludes_plugin_docs() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cchub-plugin-scan-{unique}"));
        std::fs::create_dir_all(root.join("skills").join("real")).unwrap();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(
            root.join("skills").join("real").join("SKILL.md.disabled"),
            "disabled",
        )
        .unwrap();
        std::fs::write(root.join("docs").join("guide.md"), "documentation").unwrap();

        let plan = SkillScanPlan {
            roots: vec![SkillScanRoot {
                path: root.clone(),
                plugin_id: Some("real-plugin@market".to_string()),
                tool_id: Some("claude".to_string()),
            }],
            metadata_map: HashMap::new(),
        };
        let skills = scan_local_skills_from_plan(&plan);

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "SKILL");
        assert_eq!(skills[0].plugin_id.as_deref(), Some("real-plugin@market"));
        assert!(skills[0]
            .file_path
            .as_deref()
            .is_some_and(|path| path.ends_with("SKILL.md.disabled")));
        std::fs::remove_dir_all(root).unwrap();
    }
}
