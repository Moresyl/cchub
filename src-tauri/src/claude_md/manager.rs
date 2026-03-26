use serde::{Deserialize, Serialize};
use std::fs;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use rusqlite::Connection;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClaudeMdFile {
    pub path: String,
    pub project_name: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub content_preview: String,
    pub disabled: bool,
    pub tool_id: String,
    pub tool_name: String,
    pub doc_name: String,
    pub file_name: String,
    pub scope: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClaudeMdTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub content: String,
    pub tool_id: String,
    pub tool_name: String,
    pub file_name: String,
}

struct DocSpec {
    tool_id: &'static str,
    tool_name: &'static str,
    hidden_dir: &'static str,
    file_name: &'static str,
}

const DOC_SPECS: &[DocSpec] = &[
    DocSpec {
        tool_id: "claude",
        tool_name: "Claude",
        hidden_dir: ".claude",
        file_name: "CLAUDE.md",
    },
    DocSpec {
        tool_id: "codex",
        tool_name: "Codex",
        hidden_dir: ".codex",
        file_name: "AGENTS.md",
    },
    DocSpec {
        tool_id: "gemini",
        tool_name: "Gemini",
        hidden_dir: ".gemini",
        file_name: "GEMINI.md",
    },
    DocSpec {
        tool_id: "opencode",
        tool_name: "OpenCode",
        hidden_dir: ".opencode",
        file_name: "AGENTS.md",
    },
    DocSpec {
        tool_id: "openclaw",
        tool_name: "OpenClaw",
        hidden_dir: ".openclaw",
        file_name: "AGENTS.md",
    },
];

fn discover_project_roots(conn: &Connection) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    let mut push_root = |raw_path: String| {
        let trimmed = raw_path.trim();
        if trimmed.is_empty() {
            return;
        }

        let key = trimmed.replace('\\', "/");
        if !seen.insert(key) {
            return;
        }

        let path = PathBuf::from(trimmed);
        if path.exists() {
            roots.push(path);
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

pub fn scan_claude_md_files(conn: &Connection) -> Vec<ClaudeMdFile> {
    let mut results = Vec::new();
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return results,
    };

    for spec in DOC_SPECS {
        for filename in [spec.file_name.to_string(), format!("{}.bak", spec.file_name)] {
            let global_doc = home.join(spec.hidden_dir).join(&filename);
            if global_doc.exists() {
                if let Some(entry) = read_doc_entry(&global_doc, spec, &format!("Global ({})", spec.hidden_dir), "global") {
                    results.push(entry);
                }
            }
        }
    }

    for project_root in discover_project_roots(conn) {
        let project_name = project_root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| project_root.to_string_lossy().to_string());

        for spec in DOC_SPECS {
            for filename in [spec.file_name.to_string(), format!("{}.bak", spec.file_name)] {
                let project_doc = project_root.join(&filename);
                if project_doc.exists() {
                    if let Some(entry) = read_doc_entry(&project_doc, spec, &project_name, "project") {
                        results.push(entry);
                    }
                }
            }
        }
    }

    results.sort_by(|a, b| a.path.cmp(&b.path));
    results.dedup_by(|a, b| a.path == b.path);
    results
}

fn read_doc_entry(path: &Path, spec: &DocSpec, project_name: &str, scope: &str) -> Option<ClaudeMdFile> {
    let metadata = fs::metadata(path).ok()?;
    let content = fs::read_to_string(path).ok()?;
    let modified_at = metadata.modified().ok().map(|t| {
        let datetime: chrono::DateTime<chrono::Local> = t.into();
        datetime.format("%Y-%m-%d %H:%M").to_string()
    });

    let preview = if content.len() > 200 {
        let end = content.floor_char_boundary(200);
        format!("{}...", &content[..end])
    } else {
        content.clone()
    };

    let file_name = path.file_name()?.to_string_lossy().to_string();
    let disabled = file_name.ends_with(".bak");

    Some(ClaudeMdFile {
        path: path.to_string_lossy().to_string(),
        project_name: project_name.to_string(),
        size_bytes: metadata.len(),
        modified_at,
        content_preview: preview,
        disabled,
        tool_id: spec.tool_id.to_string(),
        tool_name: spec.tool_name.to_string(),
        doc_name: spec.file_name.to_string(),
        file_name,
        scope: scope.to_string(),
    })
}

pub fn read_claude_md(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))
}

pub fn write_claude_md(path: &str, content: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    crate::utils::atomic_write_string(Path::new(path), content)
        .map_err(|e| format!("Failed to write {}: {}", path, e))
}

fn allowed_doc_name(file_name: &str) -> bool {
    DOC_SPECS.iter().any(|spec| spec.file_name == file_name)
}

fn allowed_doc_or_backup(file_name: &str) -> bool {
    allowed_doc_name(file_name)
        || DOC_SPECS
            .iter()
            .any(|spec| format!("{}.bak", spec.file_name) == file_name)
}

pub fn create_instruction_doc(dir_path: &str, file_name: &str, content: &str) -> Result<String, String> {
    if !allowed_doc_name(file_name) {
        return Err(format!("Unsupported instruction document: {}", file_name));
    }
    let path = PathBuf::from(dir_path).join(file_name);
    if path.exists() {
        return Err(format!("{} already exists in this directory", file_name));
    }
    write_claude_md(&path.to_string_lossy(), content)?;
    Ok(path.to_string_lossy().to_string())
}

pub fn create_claude_md(dir_path: &str, content: &str) -> Result<String, String> {
    create_instruction_doc(dir_path, "CLAUDE.md", content)
}

fn template_content(file_name: &str, stack_name: &str, commands: &str, style_notes: &str) -> String {
    format!(
        "# {file_name}\n\n## Project Overview\nThis is a {stack_name} project.\n\n## Development Commands\n```bash\n{commands}\n```\n\n## Code Style\n{style_notes}\n\n## Architecture\n<!-- Describe the project structure -->\n"
    )
}

pub fn get_claude_md_templates() -> Vec<ClaudeMdTemplate> {
    let mut templates = Vec::new();
    for spec in DOC_SPECS {
        templates.push(ClaudeMdTemplate {
            id: format!("{}-generic", spec.tool_id),
            name: format!("{} Generic", spec.tool_name),
            description: format!("General {} instruction document", spec.file_name),
            content: template_content(
                spec.file_name,
                "general",
                "# Install dependencies\n# Start dev server\n# Run tests\n# Build for production",
                "- Describe coding conventions\n- Note important repository rules\n- Add any agent-specific guidance",
            ),
            tool_id: spec.tool_id.to_string(),
            tool_name: spec.tool_name.to_string(),
            file_name: spec.file_name.to_string(),
        });

        templates.push(ClaudeMdTemplate {
            id: format!("{}-typescript", spec.tool_id),
            name: format!("{} TypeScript", spec.tool_name),
            description: format!("TypeScript/Node.js template for {}", spec.file_name),
            content: template_content(
                spec.file_name,
                "TypeScript/Node.js",
                "npm install          # Install dependencies\nnpm run dev          # Start dev server\nnpm run build        # Build for production\nnpm test             # Run tests\nnpm run lint         # Lint code",
                "- Use TypeScript strict mode\n- Prefer const over let\n- Use async/await over raw promises\n- Follow ESLint and formatter rules",
            ),
            tool_id: spec.tool_id.to_string(),
            tool_name: spec.tool_name.to_string(),
            file_name: spec.file_name.to_string(),
        });

        templates.push(ClaudeMdTemplate {
            id: format!("{}-rust", spec.tool_id),
            name: format!("{} Rust", spec.tool_name),
            description: format!("Rust/Cargo template for {}", spec.file_name),
            content: template_content(
                spec.file_name,
                "Rust/Cargo",
                "cargo build          # Build the project\ncargo test           # Run tests\ncargo run            # Run the project\ncargo clippy         # Lint\ncargo fmt            # Format code",
                "- Follow Rust idioms and conventions\n- Use Result<T, E> for error handling\n- Prefer explicit module boundaries\n- Write doc comments for public APIs",
            ),
            tool_id: spec.tool_id.to_string(),
            tool_name: spec.tool_name.to_string(),
            file_name: spec.file_name.to_string(),
        });

        templates.push(ClaudeMdTemplate {
            id: format!("{}-python", spec.tool_id),
            name: format!("{} Python", spec.tool_name),
            description: format!("Python template for {}", spec.file_name),
            content: template_content(
                spec.file_name,
                "Python",
                "pip install -e .     # Install in dev mode\npytest               # Run tests\npython -m mypy .     # Type check\nruff check .         # Lint\nruff format .        # Format",
                "- Follow PEP 8\n- Use type hints\n- Prefer dataclasses for data structures\n- Use virtual environments",
            ),
            tool_id: spec.tool_id.to_string(),
            tool_name: spec.tool_name.to_string(),
            file_name: spec.file_name.to_string(),
        });
    }
    templates
}

pub fn delete_claude_md(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("File not found: {}", path));
    }
    let filename = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if !allowed_doc_or_backup(&filename) {
        return Err("Can only delete supported instruction documents".to_string());
    }
    fs::remove_file(p).map_err(|e| format!("Failed to delete {}: {}", path, e))
}

pub fn disable_claude_md(path: &str) -> Result<String, String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("File not found: {}", path));
    }
    let filename = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if !allowed_doc_name(&filename) {
        return Err("Unsupported instruction document".to_string());
    }
    let new_path = format!("{}.bak", path);
    fs::rename(p, &new_path).map_err(|e| format!("Failed to disable: {}", e))?;
    Ok(new_path)
}

pub fn enable_claude_md(path: &str) -> Result<String, String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("File not found: {}", path));
    }
    let filename = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if !allowed_doc_or_backup(&filename) || !path.ends_with(".bak") {
        return Err("File is not a disabled instruction document".to_string());
    }
    let new_path = path.trim_end_matches(".bak").to_string();
    if Path::new(&new_path).exists() {
        return Err(format!("Cannot enable: {} already exists", new_path));
    }
    fs::rename(p, &new_path).map_err(|e| format!("Failed to enable: {}", e))?;
    Ok(new_path)
}
