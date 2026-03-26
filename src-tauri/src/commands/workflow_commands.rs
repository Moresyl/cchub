use crate::workflows;
use tauri::command;

#[command]
pub fn scan_workflows() -> Result<Vec<workflows::WorkflowFile>, String> {
    Ok(workflows::scan_workflow_files())
}

#[command]
pub fn get_workflow_templates() -> Result<Vec<workflows::WorkflowTemplate>, String> {
    Ok(workflows::get_workflow_templates())
}

#[command]
pub fn install_workflow(tool_id: String, template_id: String) -> Result<String, String> {
    workflows::install_workflow(&tool_id, &template_id)
}

#[command]
pub fn read_workflow_content(path: String) -> Result<String, String> {
    workflows::read_workflow(&path)
}

#[command]
pub fn write_workflow_content(path: String, content: String) -> Result<(), String> {
    workflows::write_workflow(&path, &content)
}

#[command]
pub fn delete_workflow(path: String) -> Result<(), String> {
    workflows::delete_workflow(&path)
}

#[command]
pub fn toggle_workflow(path: String, enabled: bool) -> Result<String, String> {
    workflows::toggle_workflow(&path, enabled)
}

/// Import a workflow .md file from disk to a tool's workflows directory
#[command]
pub async fn import_workflow_file(tool_id: String) -> Result<String, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Import Workflow")
        .add_filter("Markdown", &["md"])
        .pick_file()
        .await
        .ok_or("Cancelled")?;

    let source_path = file.path();
    let file_name = source_path
        .file_name()
        .ok_or("Invalid file name")?
        .to_string_lossy()
        .to_string();

    let content = std::fs::read_to_string(source_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let hidden_dir = match tool_id.as_str() {
        "claude" => ".claude",
        "codex" => ".codex",
        "gemini" => ".gemini",
        "opencode" => ".opencode",
        "openclaw" => ".openclaw",
        _ => return Err(format!("Unknown tool: {}", tool_id)),
    };

    let wf_dir = home.join(hidden_dir).join("workflows");
    std::fs::create_dir_all(&wf_dir).map_err(|e| e.to_string())?;

    let target = wf_dir.join(&file_name);
    if target.exists() {
        return Err(format!("Workflow already exists: {}", file_name));
    }

    crate::utils::atomic_write_string(&target, &content)
        .map_err(|e| format!("Failed to write: {}", e))?;

    Ok(target.to_string_lossy().to_string())
}
