//! Field-level JSON/JSONC updates. Unchanged nodes retain their original bytes.
use jsonc_parser::cst::{CstInputValue, CstNode, CstObject, CstRootNode};
use jsonc_parser::ParseOptions;
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

// Serialize this application's read/modify/write operations. External writers are
// checked again immediately before replacement; they do not participate in this lock.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn write_lock() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    WRITE_LOCK
        .lock()
        .map_err(|_| "Configuration write lock is unavailable".into())
}

fn options() -> ParseOptions {
    ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    }
}

fn check_keys(node: &CstNode) -> Result<(), String> {
    if let Some(object) = node.as_object() {
        let mut names = HashSet::new();
        for property in object.properties() {
            let name = property
                .decoded_name()
                .ok_or("Invalid JSON property name")?;
            if !names.insert(name) {
                return Err(
                    "Configuration contains duplicate fields; resolve them before editing".into(),
                );
            }
        }
    }
    for child in node.children_exclude_trivia_and_tokens() {
        check_keys(&child)?;
    }
    Ok(())
}

fn parse(source: &str) -> Result<(CstRootNode, Value), String> {
    parse_with_options(source, &options())
}

fn parse_with_options(
    source: &str,
    options: &ParseOptions,
) -> Result<(CstRootNode, Value), String> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    // Parser errors can contain credential-bearing source lines. Return a safe
    // actionable message instead of forwarding those lines to logs or the UI.
    let root = CstRootNode::parse(source, options)
        .map_err(|_| "Invalid JSON/JSONC configuration; check syntax before editing")?;
    let node = root.value().ok_or("Configuration is empty")?;
    check_keys(&node)?;
    let value: Value = jsonc_parser::parse_to_serde_value(source, options)
        .map_err(|_| "Invalid JSON/JSONC configuration; check syntax before editing")?;
    if !value.is_object() {
        return Err("Configuration root must be a JSON object".into());
    }
    Ok((root, value))
}

pub(crate) fn parse_json_object(source: &str) -> Result<Value, String> {
    parse(source).map(|(_, value)| value)
}

fn json5_options() -> ParseOptions {
    ParseOptions {
        allow_loose_object_property_names: true,
        allow_single_quoted_strings: true,
        allow_hexadecimal_numbers: true,
        allow_unary_plus_numbers: true,
        allow_bare_decimal_point_numbers: true,
        allow_non_finite_numbers: true,
        allow_extended_string_escapes: true,
        ..options()
    }
}

pub(crate) fn parse_json5_object(source: &str) -> Result<Value, String> {
    parse_with_options(source, &json5_options())
        .map(|(_, value)| value)
        .map_err(|_| {
            "Invalid JSON5 object; check syntax and duplicate fields before switching".into()
        })
}

fn input(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(value) => CstInputValue::Bool(*value),
        Value::Number(value) => CstInputValue::Number(value.to_string()),
        Value::String(value) => CstInputValue::String(value.clone()),
        Value::Array(values) => CstInputValue::Array(values.iter().map(input).collect()),
        Value::Object(values) => CstInputValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), input(value)))
                .collect(),
        ),
    }
}

fn merge(
    object: &CstObject,
    old: &serde_json::Map<String, Value>,
    desired: &serde_json::Map<String, Value>,
) {
    for property in object.properties() {
        let name = property.decoded_name().expect("validated property name");
        let Some(next) = desired.get(&name) else {
            property.remove();
            continue;
        };
        if old.get(&name) == Some(next) {
            continue;
        }
        match (
            old.get(&name).and_then(Value::as_object),
            next.as_object(),
            property.object_value(),
        ) {
            (Some(before), Some(after), Some(child)) => merge(&child, before, after),
            _ => property.set_value(input(next)),
        }
    }
    for (name, value) in desired {
        if !old.contains_key(name) {
            object.append(name, input(value));
        }
    }
}

fn render(
    source: &str,
    root: &CstRootNode,
    before: &Value,
    desired: &Value,
    json5: bool,
) -> Result<String, String> {
    let desired = desired
        .as_object()
        .ok_or("Configuration root must remain a JSON object")?;
    merge(
        &root.object_value().expect("validated object"),
        before.as_object().unwrap(),
        desired,
    );
    let mut output = root.to_string();
    if source.starts_with('\u{feff}') {
        output.insert(0, '\u{feff}');
    }
    let rendered = if json5 {
        parse_json5_object(&output)?
    } else {
        parse_json_object(&output)?
    };
    if rendered != Value::Object(desired.clone()) {
        return Err(
            "Configuration serialization did not match the intended update; no file was written"
                .into(),
        );
    }
    Ok(output)
}

/// Update a configuration under the application write lock. No-op edits leave
/// bytes and timestamps untouched. Invalid files are never replaced with defaults.
pub(crate) fn update_json_file(
    path: &Path,
    edit: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    let _guard = write_lock()?;
    let original = match std::fs::read_to_string(path) {
        Ok(source) => Some(source),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("Cannot read configuration: {error}")),
    };
    let source = original.as_deref().unwrap_or("{}\n");
    let output = edit_json_text(source, edit)?;
    if output == source {
        return Ok(());
    }
    let current = match std::fs::read_to_string(path) {
        Ok(source) => Some(source),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("Cannot recheck configuration: {error}")),
    };
    if current != original {
        return Err("Configuration changed externally; reload it and try again".into());
    }
    crate::utils::atomic_write_string(path, &output).map_err(|error| error.to_string())
}

/// Prepare an edit without writing, so a multi-file operation can preflight
/// every member before replacing any file.
pub(crate) fn edit_json_text(
    source: &str,
    edit: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Result<String, String> {
    let (root, before) = parse(source)?;
    let mut desired = before.clone();
    edit(&mut desired)?;
    if before == desired {
        return Ok(source.to_string());
    }
    render(source, &root, &before, &desired, false)
}

/// Retain JSON5 comments, unchanged nodes and extensions while editing a draft.
pub(crate) fn edit_json5_text(source: &str, desired: &Value) -> Result<String, String> {
    let (root, before) = parse_with_options(source, &json5_options())?;
    if !desired.is_object() {
        return Err("Configuration root must remain a JSON object".into());
    }
    if &before == desired {
        return Ok(source.to_owned());
    }
    render(source, &root, &before, desired, true)
}

#[cfg(test)]
#[path = "json_config_tests.rs"]
mod tests;
