use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkflowFile {
    pub path: String,
    pub tool_id: String,
    pub tool_name: String,
    pub name: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub content_preview: String,
    pub disabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkflowTemplate {
    pub id: String,
    pub name_zh: String,
    pub name_en: String,
    pub description_zh: String,
    pub description_en: String,
    pub category: String,
    pub content: String,
}

const WORKFLOW_TOOLS: &[(&str, &str, &str)] = &[
    ("claude", ".claude", "Claude"),
    ("codex", ".codex", "Codex"),
    ("gemini", ".gemini", "Gemini"),
    ("opencode", ".opencode", "OpenCode"),
    ("openclaw", ".openclaw", "OpenClaw"),
];

// ── Scan ──

pub fn scan_workflow_files() -> Vec<WorkflowFile> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let mut results = Vec::new();
    for &(tool_id, hidden_dir, tool_name) in WORKFLOW_TOOLS {
        let wf_dir = home.join(hidden_dir).join("workflows");
        if !wf_dir.exists() {
            continue;
        }
        let entries = match fs::read_dir(&wf_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();

            let is_md = fname.ends_with(".md") && !fname.ends_with(".md.disabled");
            let is_disabled = fname.ends_with(".md.disabled");
            if !is_md && !is_disabled {
                continue;
            }

            let metadata = match fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let content = fs::read_to_string(&path).unwrap_or_default();
            let modified_at = metadata.modified().ok().map(|t| {
                let dt: chrono::DateTime<chrono::Local> = t.into();
                dt.format("%Y-%m-%d %H:%M").to_string()
            });
            let preview = if content.len() > 200 {
                let end = content.floor_char_boundary(200);
                format!("{}...", &content[..end])
            } else {
                content.clone()
            };

            // Derive display name from filename
            let display_name = fname
                .trim_end_matches(".disabled")
                .trim_end_matches(".md")
                .replace('-', " ")
                .replace('_', " ");

            results.push(WorkflowFile {
                path: path.to_string_lossy().to_string(),
                tool_id: tool_id.to_string(),
                tool_name: tool_name.to_string(),
                name: display_name,
                file_name: fname,
                size_bytes: metadata.len(),
                modified_at,
                content_preview: preview,
                disabled: is_disabled,
            });
        }
    }
    results.sort_by(|a, b| a.tool_id.cmp(&b.tool_id).then(a.name.cmp(&b.name)));
    results
}

// ── CRUD ──

pub fn read_workflow(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))
}

pub fn write_workflow(path: &str, content: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    crate::utils::atomic_write_string(Path::new(path), content)
        .map_err(|e| format!("Failed to write {}: {}", path, e))
}

pub fn delete_workflow(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("File not found: {}", path));
    }
    let fname = p.file_name().unwrap_or_default().to_string_lossy().to_string();
    if !fname.ends_with(".md") && !fname.ends_with(".md.disabled") {
        return Err("Can only delete workflow .md files".to_string());
    }
    fs::remove_file(p).map_err(|e| format!("Failed to delete {}: {}", path, e))
}

pub fn toggle_workflow(path: &str, enabled: bool) -> Result<String, String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("File not found: {}", path));
    }
    let new_path = if enabled {
        // .md.disabled → .md
        path.trim_end_matches(".disabled").to_string()
    } else {
        // .md → .md.disabled
        format!("{}.disabled", path)
    };
    if Path::new(&new_path).exists() && new_path != path {
        return Err(format!("Target already exists: {}", new_path));
    }
    fs::rename(p, &new_path).map_err(|e| format!("Failed to toggle: {}", e))?;
    Ok(new_path)
}

// ── Install template ──

pub fn install_workflow(tool_id: &str, template_id: &str) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    let hidden_dir = WORKFLOW_TOOLS
        .iter()
        .find(|&&(id, _, _)| id == tool_id)
        .map(|&(_, dir, _)| dir)
        .ok_or(format!("Unknown tool: {}", tool_id))?;

    let templates = get_workflow_templates();
    let tmpl = templates
        .iter()
        .find(|t| t.id == template_id)
        .ok_or(format!("Unknown template: {}", template_id))?;

    let wf_dir = home.join(hidden_dir).join("workflows");
    fs::create_dir_all(&wf_dir).map_err(|e| e.to_string())?;

    let file_name = format!("{}.md", template_id);
    let file_path = wf_dir.join(&file_name);
    if file_path.exists() {
        return Err(format!("Workflow already exists: {}", file_name));
    }

    crate::utils::atomic_write_string(&file_path, &tmpl.content)
        .map_err(|e| format!("Failed to write: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}

// ── Templates ──

pub fn get_workflow_templates() -> Vec<WorkflowTemplate> {
    vec![
        WorkflowTemplate {
            id: "code-review".into(),
            name_zh: "代码审查".into(),
            name_en: "Code Review".into(),
            description_zh: "PR/代码审查规范与检查清单".into(),
            description_en: "PR/code review standards and checklist".into(),
            category: "quality".into(),
            content: include_str!("templates/code-review.md").into(),
        },
        WorkflowTemplate {
            id: "tdd".into(),
            name_zh: "测试驱动开发".into(),
            name_en: "TDD Workflow".into(),
            description_zh: "Red-Green-Refactor 循环".into(),
            description_en: "Red-Green-Refactor cycle".into(),
            category: "development".into(),
            content: include_str!("templates/tdd.md").into(),
        },
        WorkflowTemplate {
            id: "bug-diagnosis".into(),
            name_zh: "Bug 诊断".into(),
            name_en: "Bug Diagnosis".into(),
            description_zh: "系统化调试方法论".into(),
            description_en: "Systematic debugging methodology".into(),
            category: "debugging".into(),
            content: include_str!("templates/bug-diagnosis.md").into(),
        },
        WorkflowTemplate {
            id: "refactoring".into(),
            name_zh: "重构指南".into(),
            name_en: "Refactoring Guide".into(),
            description_zh: "安全重构模式与步骤".into(),
            description_en: "Safe refactoring patterns and steps".into(),
            category: "development".into(),
            content: include_str!("templates/refactoring.md").into(),
        },
        WorkflowTemplate {
            id: "commit-convention".into(),
            name_zh: "提交规范".into(),
            name_en: "Commit Convention".into(),
            description_zh: "Git 提交信息标准".into(),
            description_en: "Git commit message standards".into(),
            category: "workflow".into(),
            content: include_str!("templates/commit-convention.md").into(),
        },
        WorkflowTemplate {
            id: "security-audit".into(),
            name_zh: "安全审计".into(),
            name_en: "Security Audit".into(),
            description_zh: "OWASP Top 10 安全检查".into(),
            description_en: "OWASP Top 10 security checklist".into(),
            category: "security".into(),
            content: include_str!("templates/security-audit.md").into(),
        },
        WorkflowTemplate {
            id: "performance".into(),
            name_zh: "性能优化".into(),
            name_en: "Performance Optimization".into(),
            description_zh: "性能分析与优化策略".into(),
            description_en: "Performance analysis and optimization strategies".into(),
            category: "optimization".into(),
            content: include_str!("templates/performance.md").into(),
        },
        WorkflowTemplate {
            id: "documentation".into(),
            name_zh: "文档生成".into(),
            name_en: "Documentation".into(),
            description_zh: "自动化文档生成规范".into(),
            description_en: "Automated documentation generation standards".into(),
            category: "documentation".into(),
            content: include_str!("templates/documentation.md").into(),
        },
        WorkflowTemplate {
            id: "api-design".into(),
            name_zh: "API 设计".into(),
            name_en: "API Design".into(),
            description_zh: "RESTful API 设计模式".into(),
            description_en: "RESTful API design patterns".into(),
            category: "architecture".into(),
            content: include_str!("templates/api-design.md").into(),
        },
        WorkflowTemplate {
            id: "db-migration".into(),
            name_zh: "数据库迁移".into(),
            name_en: "DB Migration".into(),
            description_zh: "安全的数据库变更流程".into(),
            description_en: "Safe database change workflow".into(),
            category: "database".into(),
            content: include_str!("templates/db-migration.md").into(),
        },
        WorkflowTemplate {
            id: "ci-cd".into(),
            name_zh: "CI/CD 流水线".into(),
            name_en: "CI/CD Pipeline".into(),
            description_zh: "持续集成部署最佳实践".into(),
            description_en: "Continuous integration and deployment best practices".into(),
            category: "devops".into(),
            content: include_str!("templates/ci-cd.md").into(),
        },
        WorkflowTemplate {
            id: "code-generation".into(),
            name_zh: "代码生成".into(),
            name_en: "Code Generation".into(),
            description_zh: "AI 辅助代码生成规范".into(),
            description_en: "AI-assisted code generation standards".into(),
            category: "ai".into(),
            content: include_str!("templates/code-generation.md").into(),
        },
    ]
}
