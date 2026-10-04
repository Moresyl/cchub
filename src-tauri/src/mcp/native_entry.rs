//! Full native entry changes for catalog ownership operations. Connection forms
//! first use the existing adapter; this layer retains the complete native spec.
use super::native_read::Entry;
use super::sources::{NativeSpec, SourceBinding};
use crate::config_write::{FilePlan, FileUpdate};
use std::collections::BTreeMap;
use std::path::PathBuf;

mod connection;
mod toml;
pub(crate) use connection::{patch_connection, project_connection};

pub(crate) struct Change {
    pub binding: SourceBinding,
    pub canonical_path: PathBuf,
    pub container: String,
    pub name: String,
    pub spec: Option<NativeSpec>,
    pub original: Option<Vec<u8>>,
    pub revision: crate::config_write::FileRevision,
    pub aliases: Vec<SourceBinding>,
}

pub(crate) struct PreparedNative {
    pub plan: FilePlan,
    bindings: Vec<(SourceBinding, PathBuf)>,
    revisions: Vec<(crate::config_write::FileRevision, bool)>,
}

impl PreparedNative {
    pub(crate) fn guard_source(
        &mut self,
        origin: &super::sources::NativeOrigin,
        snapshot: &super::sources::SourceSnapshot,
    ) {
        self.revisions
            .push((snapshot.documents[0].revision.clone(), false));
        self.bindings.extend(
            origin
                .bindings
                .iter()
                .cloned()
                .map(|binding| (binding, origin.canonical_path.clone())),
        );
    }

    pub(crate) fn commit_then(
        self,
        finalize: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let Self {
            plan,
            bindings,
            revisions,
        } = self;
        for (revision, _) in &revisions {
            revision.verify()?;
        }
        let verify = || -> Result<(), String> {
            for (binding, canonical) in &bindings {
                if crate::config_write::target_key(&binding.path)? != *canonical {
                    return Err("MCP source location changed; refresh before continuing".into());
                }
            }
            Ok(())
        };
        verify()?;
        let captured = revisions
            .iter()
            .map(|(revision, _)| revision.clone())
            .collect();
        plan.commit_then_with_revisions(captured, || {
            verify()?;
            for (revision, updated) in &revisions {
                if *updated {
                    revision.verify_parents()?;
                } else {
                    revision.verify()?;
                }
            }
            finalize()
        })
    }
}

fn json(source: &str, changes: &[&Change]) -> Result<String, String> {
    crate::json_config::edit_json_text(source, |root| {
        let root = root
            .as_object_mut()
            .ok_or("Native MCP JSON must be an object")?;
        for change in changes {
            let desired = change
                .spec
                .as_ref()
                .map(|spec| match spec.entry()? {
                    Entry::Json(fields) => Ok(serde_json::Value::Object(fields)),
                    _ => Err("MCP native format changed".to_owned()),
                })
                .transpose()?;
            let entries = if change.container.is_empty() {
                &mut *root
            } else {
                if !root.contains_key(&change.container) && desired.is_none() {
                    continue;
                }
                root.entry(&change.container)
                    .or_insert_with(|| serde_json::json!({}))
                    .as_object_mut()
                    .ok_or("Native MCP container must be an object")?
            };
            if let Some(desired) = desired {
                entries.insert(change.name.clone(), desired);
            } else {
                entries.remove(&change.name);
            }
        }
        Ok(())
    })
}

fn yaml(source: &str, changes: &[&Change]) -> Result<String, String> {
    crate::yaml_config::edit_yaml_text(source, |root| {
        let mut effective = serde_yaml::Value::Mapping(root.clone());
        effective
            .apply_merge()
            .map_err(|_| "Invalid native MCP YAML merge")?;
        let key = serde_yaml::Value::String("mcp_servers".into());
        // Materialize current inherited options when an explicit container is
        // needed; adding a partial root override must not hide its siblings.
        let effective_entries = effective
            .get("mcp_servers")
            .and_then(serde_yaml::Value::as_mapping)
            .cloned()
            .unwrap_or_default();
        let merged_container = root
            .get(&key)
            .and_then(serde_yaml::Value::as_mapping)
            .is_some_and(|entries| entries.contains_key(serde_yaml::Value::String("<<".into())));
        if (!root.contains_key(&key) && !effective_entries.is_empty()) || merged_container {
            root.insert(key.clone(), serde_yaml::Value::Mapping(effective_entries));
        }
        for change in changes {
            let desired = change
                .spec
                .as_ref()
                .map(|spec| match spec.entry()? {
                    Entry::Yaml(fields) => Ok(serde_yaml::Value::Mapping(fields)),
                    _ => Err("MCP native format changed".to_owned()),
                })
                .transpose()?;
            if !root.contains_key(&key) && desired.is_none() {
                continue;
            }
            let entries = root
                .entry(key.clone())
                .or_insert_with(|| serde_yaml::Value::Mapping(Default::default()))
                .as_mapping_mut()
                .ok_or("Native MCP container must be a mapping")?;
            let name = serde_yaml::Value::String(change.name.clone());
            if let Some(desired) = desired {
                entries.insert(name, desired);
            } else {
                entries.remove(name);
            }
        }
        Ok(())
    })
}

pub(crate) fn prepare(changes: &[Change]) -> Result<PreparedNative, String> {
    let mut groups: BTreeMap<PathBuf, Vec<&Change>> = BTreeMap::new();
    let mut bindings = Vec::new();
    for change in changes {
        if change.name.trim().is_empty() || change.name.chars().any(char::is_control) {
            return Err(
                "MCP server names must be nonempty and contain no control characters".into(),
            );
        }
        if crate::config_write::target_key(&change.binding.path)? != change.canonical_path {
            return Err("MCP source location changed; refresh before continuing".into());
        }
        if let Some(spec) = &change.spec {
            super::native_read::validate_entry(&change.name, &spec.entry()?, &change.binding.tool)?;
        }
        bindings.push((change.binding.clone(), change.canonical_path.clone()));
        for alias in &change.aliases {
            if crate::config_write::target_key(&alias.path)? != change.canonical_path {
                return Err("MCP source alias location changed; refresh before continuing".into());
            }
            bindings.push((alias.clone(), change.canonical_path.clone()));
        }
        groups
            .entry(change.canonical_path.clone())
            .or_default()
            .push(change);
    }
    let mut plan = FilePlan::default();
    let mut revisions = Vec::new();
    for (path, changes) in groups {
        for change in &changes {
            change.revision.verify()?;
        }
        let mut targets = BTreeMap::new();
        for change in &changes {
            if let Some(old) = targets.insert((&change.container, &change.name), &change.spec) {
                if old != &change.spec {
                    return Err("Conflicting MCP changes to one native entry".into());
                }
            }
        }
        let original = crate::config_write::read(&path)?;
        if changes.iter().any(|change| change.original != original) {
            return Err("MCP configuration changed externally; refresh before continuing".into());
        }
        let is_toml = changes
            .iter()
            .any(|change| matches!(change.binding.tool.as_str(), "codex" | "grokbuild"));
        let is_yaml = changes.iter().any(|change| change.binding.tool == "hermes");
        if changes.iter().any(|change| {
            matches!(change.binding.tool.as_str(), "codex" | "grokbuild") != is_toml
                || (change.binding.tool == "hermes") != is_yaml
        }) {
            return Err("MCP aliases require incompatible native document formats".into());
        }
        let empty = if is_toml { "" } else { "{}\n" };
        let source = original
            .as_deref()
            .map(std::str::from_utf8)
            .transpose()
            .map_err(|_| "MCP native configuration must use UTF-8")?
            .unwrap_or(empty);
        for change in &changes {
            super::native_read::parse_entries(
                source,
                &change.binding.tool,
                change.container.is_empty(),
            )?;
        }
        let desired = if is_toml {
            toml::edit(source, &changes)?
        } else if is_yaml {
            yaml(source, &changes)?
        } else {
            json(source, &changes)?
        };
        // Validate every requested scope against the rendered document, not just
        // a selected definition. Existing invalid siblings cannot be ignored.
        for change in &changes {
            let (container, entries) = super::native_read::parse_entries(
                &desired,
                &change.binding.tool,
                change.container.is_empty(),
            )?;
            if container != change.container {
                return Err("MCP native container changed unexpectedly".into());
            }
            let actual = entries
                .get(&change.name)
                .map(NativeSpec::from_entry)
                .transpose()?;
            let matches = match (&actual, &change.spec) {
                (None, None) => true,
                (Some(actual), Some(expected)) => actual.same(expected)?,
                _ => false,
            };
            if !matches {
                return Err("MCP native change did not produce the intended entry".into());
            }
        }
        if desired.as_bytes() == source.as_bytes() {
            revisions.push((changes[0].revision.clone(), false));
            plan.guards.push((path, original));
        } else {
            revisions.push((changes[0].revision.clone(), true));
            plan.updates.push(FileUpdate {
                path,
                original,
                desired: desired.into_bytes(),
            });
        }
    }
    Ok(PreparedNative {
        plan,
        bindings,
        revisions,
    })
}

#[cfg(test)]
mod tests;
