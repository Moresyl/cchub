//! Lossless selected edits to one native YAML document.
use serde_yaml::{Mapping, Value};
use std::str::FromStr;

fn invalid() -> String {
    "Invalid native YAML; check syntax and mapping types before editing".into()
}

pub(crate) fn same(left: &Value, right: &Value) -> bool {
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
        return Err("Unsupported native YAML sequence layout".into());
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
        return Err("Cannot retain native YAML sequence trivia".into());
    }
    Ok(())
}

fn normalize_changed_merge_key(target: &yaml_edit::Mapping) -> Result<(), String> {
    use yaml_edit::{AsYaml, SyntaxKind};
    // The CST library does not recognize a MERGE_KEY token as a scalar key,
    // so remove("<<") silently leaves the inheritance in place. Give that
    // selected key a normal quoted KEY node before its standard field edit.
    let syntax = target.as_node().ok_or_else(invalid)?;
    let quoted = yaml_edit::YamlFile::from_str("\"<<\": null\n").map_err(|_| invalid())?;
    let key_node = quoted
        .document()
        .and_then(|doc| doc.as_mapping())
        .and_then(|mapping| mapping.as_node().cloned())
        .and_then(|mapping| {
            mapping
                .children()
                .find(|entry| entry.kind() == SyntaxKind::MAPPING_ENTRY)
        })
        .and_then(|entry| entry.children().find(|node| node.kind() == SyntaxKind::KEY))
        .ok_or_else(invalid)?;
    for entry in syntax
        .children()
        .filter(|node| node.kind() == SyntaxKind::MAPPING_ENTRY)
    {
        let children = entry.children_with_tokens().collect::<Vec<_>>();
        let Some(index) = children.iter().position(|child| {
            child.as_node().is_some_and(|node| {
                node.kind() == SyntaxKind::KEY
                    && node
                        .descendants_with_tokens()
                        .any(|token| token.kind() == SyntaxKind::MERGE_KEY)
            })
        }) else {
            continue;
        };
        entry.splice_children(index..index + 1, vec![key_node.clone().into()]);
    }
    Ok(())
}

fn patch(target: &yaml_edit::Mapping, before: &Mapping, desired: &Mapping) -> Result<(), String> {
    let merge_key = Value::String("<<".into());
    if before
        .get(&merge_key)
        .is_some_and(|old| desired.get(&merge_key).is_none_or(|next| !same(old, next)))
    {
        normalize_changed_merge_key(target)?;
    }
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
                        return Err("Cannot update native YAML sequence".into());
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

pub(crate) fn edit_yaml_text(
    source: &str,
    edit: impl FnOnce(&mut Mapping) -> Result<(), String>,
) -> Result<String, String> {
    let input = source.strip_prefix('\u{feff}').unwrap_or(source);
    let original: Value = serde_yaml::from_str(input).map_err(|_| invalid())?;
    let mut expected = original.clone();
    edit(expected.as_mapping_mut().ok_or_else(invalid)?)?;
    expected.as_mapping().ok_or_else(invalid)?;
    if same(&original, &expected) {
        return Ok(source.into());
    }
    let file = yaml_edit::YamlFile::from_str(input).map_err(|_| invalid())?;
    if file.documents().count() != 1 {
        return Err("Native YAML must contain a single document".into());
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
        serde_yaml::from_str(&output).map_err(|_| "Cannot encode native YAML without loss")?;
    if !same(&actual, &expected) {
        return Err("Native YAML edit did not match the intended configuration".into());
    }
    Ok(if source.starts_with('\u{feff}') {
        format!("\u{feff}{output}")
    } else {
        output
    })
}

#[cfg(test)]
mod tests;
