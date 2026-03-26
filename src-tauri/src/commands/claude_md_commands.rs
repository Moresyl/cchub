use crate::claude_md::manager;
use crate::db::DbState;
use tauri::command;
use tauri::State;

#[command]
pub fn scan_claude_md(db: State<'_, DbState>) -> Result<Vec<manager::ClaudeMdFile>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(manager::scan_claude_md_files(&conn))
}

#[command]
pub fn read_claude_md_content(path: String) -> Result<String, String> {
    manager::read_claude_md(&path)
}

#[command]
pub fn write_claude_md_content(path: String, content: String) -> Result<(), String> {
    manager::write_claude_md(&path, &content)
}

#[command]
pub fn get_claude_md_templates() -> Result<Vec<manager::ClaudeMdTemplate>, String> {
    Ok(manager::get_claude_md_templates())
}

#[command]
pub fn create_new_claude_md(dir_path: String, content: String) -> Result<String, String> {
    manager::create_claude_md(&dir_path, &content)
}

#[command]
pub fn create_instruction_doc_file(
    dir_path: String,
    file_name: String,
    content: String,
) -> Result<String, String> {
    manager::create_instruction_doc(&dir_path, &file_name, &content)
}

#[command]
pub fn delete_claude_md_file(path: String) -> Result<(), String> {
    manager::delete_claude_md(&path)
}

#[command]
pub fn disable_claude_md_file(path: String) -> Result<String, String> {
    manager::disable_claude_md(&path)
}

#[command]
pub fn enable_claude_md_file(path: String) -> Result<String, String> {
    manager::enable_claude_md(&path)
}
