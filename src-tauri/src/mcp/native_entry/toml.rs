use super::Change;
use crate::mcp::native_read::Entry;
use toml_edit::{DocumentMut, Item};

pub(super) fn edit(source: &str, changes: &[&Change]) -> Result<String, String> {
    let input = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut document: DocumentMut = input.parse().map_err(|_| "Invalid native MCP TOML")?;
    let mut expected: toml::Table = toml::from_str(input).map_err(|_| "Invalid native MCP TOML")?;
    let before = expected.clone();
    for change in changes {
        let desired = change
            .spec
            .as_ref()
            .map(|spec| match spec.entry()? {
                Entry::Toml(fields) => Ok(fields),
                _ => Err("MCP native format changed".to_owned()),
            })
            .transpose()?;
        if document.get("mcp_servers").is_none() && desired.is_none() {
            continue;
        }
        if document.get("mcp_servers").is_none() {
            document["mcp_servers"] = toml_edit::table();
            expected.insert("mcp_servers".into(), toml::Value::Table(Default::default()));
        }
        let entries = document["mcp_servers"]
            .as_table_like_mut()
            .ok_or("Native MCP container must be a table")?;
        let values = expected
            .get_mut("mcp_servers")
            .and_then(toml::Value::as_table_mut)
            .ok_or("Native MCP container must be a table")?;
        if let Some(desired) = desired {
            let encoded =
                toml::to_string(&desired).map_err(|_| "Cannot encode native MCP definition")?;
            let generated: DocumentMut = encoded
                .parse()
                .map_err(|_| "Cannot encode native MCP definition")?;
            if let Some(existing) = entries.get_mut(&change.name) {
                let old = values
                    .get(&change.name)
                    .and_then(toml::Value::as_table)
                    .ok_or("Native MCP definition must be a table")?;
                let keys = old
                    .keys()
                    .chain(desired.keys())
                    .map(String::as_str)
                    .collect::<std::collections::BTreeSet<_>>();
                crate::mcp::native_toml::patch_fields(
                    existing
                        .as_table_like_mut()
                        .ok_or("Native MCP definition must be a table")?,
                    old,
                    &desired,
                    generated.as_table(),
                    &keys.into_iter().collect::<Vec<_>>(),
                );
            } else {
                entries.insert(&change.name, Item::Table(generated.as_table().clone()));
            }
            values.insert(change.name.clone(), toml::Value::Table(desired));
        } else {
            entries.remove(&change.name);
            values.remove(&change.name);
        }
    }
    if crate::mcp::native_toml::same_table(&before, &expected) {
        return Ok(source.into());
    }
    let mut output = document.to_string();
    if input.contains("\r\n") && !input.replace("\r\n", "").contains('\n') {
        output = output.replace("\r\n", "\n").replace('\n', "\r\n");
    }
    let actual: toml::Table =
        toml::from_str(&output).map_err(|_| "Cannot encode native MCP document")?;
    if !crate::mcp::native_toml::same_table(&expected, &actual) {
        return Err("Native MCP edit changed unintended values".into());
    }
    Ok(if source.starts_with('\u{feff}') {
        format!("\u{feff}{output}")
    } else {
        output
    })
}
