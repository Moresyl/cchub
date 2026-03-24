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
