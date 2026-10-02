//! Prepared Hermes MCP edits. Keep the full YAML stream and unmanaged values.
use super::config::McpServerConfig;
use crate::config_write::{FilePlan, FileUpdate};
use serde_yaml::{Mapping, Value};
use std::path::Path;
use std::str::FromStr;

const FIELDS: &[&str] = &[
    "command",
    "args",
    "env",
    "url",
    "headers",
    "type",
    "transport",
];

pub(crate) struct Edit<'a> {
    pub name: &'a str,
    pub config: Option<&'a McpServerConfig>,
}

fn key(name: &str) -> Value {
    Value::String(name.into())
}

fn invalid() -> String {
    "Invalid native MCP YAML; check syntax and mapping types before editing".into()
}

fn connection(name: &str, config: &McpServerConfig) -> Result<Mapping, String> {
    if config.command.trim().is_empty() || config.command.contains('\0') {
        return Err("MCP command or URL must be nonempty and contain no null characters".into());
    }
    if config.transport_type.as_deref().is_some_and(|transport| {
        !matches!(
            transport,
            "stdio" | "local" | "http" | "sse" | "remote" | "streamable-http"
        )
    }) {
        return Err("Unsupported MCP transport".into());
    }
    if config.args.iter().any(|arg| arg.contains('\0')) {
        return Err("MCP arguments must not contain null characters".into());
    }
    let mut fields = Mapping::new();
    let remote = super::formats::is_remote(config);
    fields.insert(
        key(if remote { "url" } else { "command" }),
        key(&config.command),
    );
    if remote && config.transport_type.as_deref() == Some("sse") {
        fields.insert(key("transport"), key("sse"));
    }
    if !remote && !config.args.is_empty() {
        fields.insert(
            key("args"),
            Value::Sequence(config.args.iter().map(|arg| key(arg)).collect()),
        );
    }
    if !config.env.is_empty() {
        // Deterministic new fields; existing native field order stays untouched.
        let values: std::collections::BTreeMap<_, _> = config.env.iter().collect();
        let values = values
            .into_iter()
            .map(|(name, value)| (key(name), key(value)))
            .collect();
        fields.insert(
            key(if remote { "headers" } else { "env" }),
            Value::Mapping(values),
        );
    }
    super::native_read::validate_yaml_entry(name, &fields)?;
    Ok(fields)
}

fn apply(root: &mut Mapping, edit: &Edit<'_>) -> Result<(), String> {
    if edit.name.trim().is_empty() || edit.name.chars().any(char::is_control) {
        return Err("MCP server name must be nonempty and contain no control characters".into());
    }
    let desired = edit
        .config
        .map(|config| connection(edit.name, config))
        .transpose()?;
    if root
        .get(key("mcp_servers"))
        .is_some_and(|value| !value.is_mapping())
    {
        return Err("MCP mcp_servers must be a YAML mapping".into());
    }
    if desired.is_none() && !root.contains_key(key("mcp_servers")) {
        return Ok(());
    }
    let servers = root
        .entry(key("mcp_servers"))
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    let servers = servers.as_mapping_mut().ok_or_else(invalid)?;
    if servers
        .get(key(edit.name))
        .is_some_and(|value| !value.is_mapping())
    {
        return Err("Existing MCP entry must be a YAML mapping".into());
    }
    if let Some(desired) = desired {
        let mut next = servers
            .get(key(edit.name))
            .and_then(Value::as_mapping)
            .cloned()
            .unwrap_or_default();
        for field in FIELDS {
            next.remove(key(field));
        }
        next.extend(desired);
        // Validate known fields without coercing unknown YAML tags, NaN or keys
        // to JSON. Timeout, enabled, auth and tool policies are never generated.
        super::native_read::validate_yaml_entry(edit.name, &next)?;
        servers.insert(key(edit.name), Value::Mapping(next));
    } else {
        servers.remove(key(edit.name));
    }
    Ok(())
}

fn same(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            left == right
                || (left.as_f64().is_some_and(f64::is_nan)
                    && right.as_f64().is_some_and(f64::is_nan))
        }
        (Value::Sequence(left), Value::Sequence(right)) => {
            left.len() == right.len() && left.iter().zip(right).all(|(a, b)| same(a, b))
        }
        (Value::Mapping(left), Value::Mapping(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .all(|(key, value)| right.get(key).is_some_and(|other| same(value, other)))
        }
        (Value::Tagged(left), Value::Tagged(right)) => {
            left.tag == right.tag && same(&left.value, &right.value)
        }
        _ => left == right,
    }
}

fn node(value: &Value) -> Result<yaml_edit::YamlNode, String> {
    // Insert a flow value: copying a block node into an existing flow mapping
    // is not supported by the CST library. Quote strings even inside arrays.
    let text = format!("value: {}\n", flow(value)?);
    let file = yaml_edit::YamlFile::from_str(&text).map_err(|_| invalid())?;
    file.document()
        .and_then(|doc| doc.get("value"))
        .ok_or_else(invalid)
}

fn flow(value: &Value) -> Result<String, String> {
    match value {
        Value::String(value) => serde_json::to_string(value).map_err(|_| invalid()),
        Value::Sequence(values) => Ok(format!(
            "[{}]",
            values
                .iter()
                .map(flow)
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        )),
        Value::Mapping(values) => {
            let fields = values
                .iter()
                .map(|(key, value)| Ok(format!("{}: {}", flow(key)?, flow(value)?)))
                .collect::<Result<Vec<_>, String>>()?;
            Ok(format!("{{{}}}", fields.join(", ")))
        }
        Value::Tagged(value) => Ok(format!("{} {}", value.tag, flow(&value.value)?)),
        _ => serde_yaml::to_string(value)
            .map(|text| text.trim_end_matches('\n').to_owned())
            .map_err(|_| invalid()),
    }
}

fn attach_sequence_tail(sequence: &yaml_edit::Sequence) -> Result<(), String> {
    use yaml_edit::{AsYaml, SyntaxKind};
    if sequence.is_flow_style() {
        return Ok(());
    }
    let syntax = sequence.as_node().ok_or_else(invalid)?;
    let stream = syntax.ancestors().last().ok_or_else(invalid)?;
    let original = stream.to_string();
    let children: Vec<_> = syntax.children_with_tokens().collect();
    let Some(index) = children
        .iter()
        .rposition(|child| child.kind() == SyntaxKind::SEQUENCE_ENTRY)
    else {
        return Ok(());
    };
    let last = children[index].as_node().ok_or_else(invalid)?.clone();
    let mut tail = children[index + 1..].to_vec();
    syntax.splice_children(index + 1..children.len(), Vec::new());
    // The parser may put the last item's inline comment outside the sequence,
    // after its containing mapping entry. Attach that same trivia to the item
    // before appending, or the new argument would swallow the old comment.
    let entry = syntax
        .parent()
        .and_then(|value| value.parent())
        .ok_or_else(invalid)?;
    let mapping = entry.parent().ok_or_else(invalid)?;
    if entry.kind() != SyntaxKind::MAPPING_ENTRY || mapping.kind() != SyntaxKind::MAPPING {
        return Err("Unsupported native MCP YAML sequence layout".into());
    }
    let siblings: Vec<_> = mapping.children_with_tokens().collect();
    let entry_index = siblings
        .iter()
        .position(|child| child.as_node() == Some(&entry))
        .ok_or_else(invalid)?;
    let mut end = entry_index + 1;
    while end < siblings.len()
        && matches!(
            siblings[end].kind(),
            SyntaxKind::COMMENT | SyntaxKind::NEWLINE | SyntaxKind::WHITESPACE
        )
    {
        end += 1;
        if siblings[end - 1].kind() == SyntaxKind::NEWLINE {
            break;
        }
    }
    tail.extend_from_slice(&siblings[entry_index + 1..end]);
    mapping.splice_children(entry_index + 1..end, Vec::new());
    let end = last.children_with_tokens().count();
    last.splice_children(end..end, tail);
    if stream.to_string() != original {
        return Err("Cannot retain native MCP YAML sequence trivia".into());
    }
    Ok(())
}

fn patch(target: &yaml_edit::Mapping, before: &Mapping, desired: &Mapping) -> Result<(), String> {
    // Set before removing, so an empty nested mapping does not detach a view
    // midway through replacing its final key with another key.
    for (name, value) in desired {
        if before.get(name).is_some_and(|old| same(old, value)) {
            continue;
        }
        let name_text = name.as_str().ok_or_else(invalid)?;
        if let (Some(old), Some(next), Some(mapping)) = (
            before.get(name).and_then(Value::as_mapping),
            value.as_mapping(),
            target.get_mapping(name_text),
        ) {
            patch(&mapping, old, next)?;
        } else if let (Some(old), Some(next), Some(sequence)) = (
            before.get(name).and_then(Value::as_sequence),
            value.as_sequence(),
            target.get_sequence(name_text),
        ) {
            attach_sequence_tail(&sequence)?;
            for (index, value) in next.iter().enumerate() {
                if old.get(index).is_some_and(|previous| same(previous, value)) {
                    continue;
                }
                if index < old.len() {
                    if !sequence.set(index, node(value)?) {
                        return Err("Cannot update native MCP YAML sequence".into());
                    }
                } else {
                    sequence.push(node(value)?);
                }
            }
            for index in (next.len()..old.len()).rev() {
                sequence.remove(index);
            }
        } else {
            target.set(name_text, node(value)?);
        }
    }
    for name in before.keys().filter(|name| !desired.contains_key(*name)) {
        target.remove(name.as_str().ok_or_else(invalid)?);
    }
    Ok(())
}

pub(crate) fn edit_text(source: &str, edits: &[Edit<'_>]) -> Result<String, String> {
    let input = source.strip_prefix('\u{feff}').unwrap_or(source);
    let original: Value = serde_yaml::from_str(input).map_err(|_| invalid())?;
    let mut expected = original.clone();
    for edit in edits {
        apply(expected.as_mapping_mut().ok_or_else(invalid)?, edit)?;
    }
    expected.as_mapping().ok_or_else(invalid)?;
    if same(&original, &expected) {
        return Ok(source.into());
    }
    let file = yaml_edit::YamlFile::from_str(input).map_err(|_| invalid())?;
    if file.documents().count() != 1 {
        return Err("Native MCP YAML must contain a single document".into());
    }
    let target = file
        .document()
        .and_then(|doc| doc.as_mapping())
        .ok_or_else(invalid)?;
    patch(
        &target,
        original.as_mapping().ok_or_else(invalid)?,
        expected.as_mapping().ok_or_else(invalid)?,
    )?;
    let mut output = file.to_string();
    if input.contains("\r\n") && !input.replace("\r\n", "").contains('\n') {
        output = output.replace("\r\n", "\n").replace('\n', "\r\n");
    }
    let actual: Value =
        serde_yaml::from_str(&output).map_err(|_| "Cannot encode native MCP YAML without loss")?;
    if !same(&actual, &expected) {
        return Err("Native MCP YAML edit did not match the intended configuration".into());
    }
    Ok(if source.starts_with('\u{feff}') {
        format!("\u{feff}{output}")
    } else {
        output
    })
}

// Pure preparation joins the caller's application-lock-before-DB operation.
pub(crate) fn prepare(path: &Path, edits: &[Edit<'_>]) -> Result<FilePlan, String> {
    let original = crate::config_write::read(path)?;
    let source = original
        .as_deref()
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|_| "Native MCP configuration must be UTF-8")?
        .unwrap_or("{}\n");
    let desired = edit_text(source, edits)?.into_bytes();
    let mut plan = FilePlan::default();
    if original.as_deref() == Some(&desired) || desired == source.as_bytes() {
        plan.guards.push((path.to_owned(), original));
    } else {
        plan.updates.push(FileUpdate {
            path: path.to_owned(),
            original,
            desired,
        });
    }
    Ok(plan)
}

pub(crate) fn update_at(
    path: &Path,
    name: &str,
    config: Option<&McpServerConfig>,
) -> Result<(), String> {
    let _guard = crate::json_config::write_lock()?;
    prepare(path, &[Edit { name, config }])?.commit()
}

#[cfg(test)]
mod tests;
