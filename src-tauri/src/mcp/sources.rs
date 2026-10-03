//! Complete, read-only native source discovery for the catalog migration.
//! Logical identity does not use a display name or imply projection ownership.
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

mod connection;
mod scopes;
mod spec;
pub use connection::ConnectionFields;
pub use spec::NativeSpec;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum SourceRole {
    Primary,
    Secondary,
    Plugin,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SourceBinding {
    pub tool: String,
    pub path: PathBuf,
    pub role: SourceRole,
}

#[derive(Debug)]
pub struct SourceDocument {
    pub canonical_path: PathBuf,
    pub original: Option<Vec<u8>>,
    pub bindings: Vec<SourceBinding>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NativeOrigin {
    pub id: String,
    pub native_name: String,
    pub canonical_path: PathBuf,
    pub container: String,
    pub bindings: Vec<SourceBinding>,
    pub connection: ConnectionFields,
    pub disabled: bool,
    pub spec: NativeSpec,
}

impl NativeOrigin {
    pub(in crate::mcp) fn new(
        binding: SourceBinding,
        canonical_path: PathBuf,
        container: String,
        name: String,
        spec: NativeSpec,
    ) -> Result<Self, String> {
        let entry = spec.entry()?;
        super::native_read::validate_entry(&name, &entry, &binding.tool)?;
        let connection = connection::read(&entry, &binding.tool)?;
        let disabled = entry.field("enabled")? == Some(serde_json::Value::Bool(false))
            || entry.field("disabled")? == Some(serde_json::Value::Bool(true));
        let origin = Self {
            id: logical_id(&canonical_path, &container, &name)?,
            native_name: name,
            canonical_path,
            container,
            bindings: vec![binding],
            connection,
            disabled,
            spec,
        };
        origin.validate()?;
        Ok(origin)
    }

    pub(in crate::mcp) fn validate(&self) -> Result<(), String> {
        if !self.canonical_path.is_absolute()
            || self.bindings.is_empty()
            || self.id != logical_id(&self.canonical_path, &self.container, &self.native_name)?
        {
            return Err("Invalid retained MCP source identity".into());
        }
        let entry = self.spec.entry()?;
        for binding in &self.bindings {
            let format = super::native_read::Format::for_tool(&binding.tool)?;
            self.spec.validate_format(&binding.tool)?;
            if !binding.path.is_absolute()
                || (self.container != format.container()
                    && !(self.container.is_empty()
                        && binding.role == SourceRole::Plugin
                        && binding.tool == "claude"))
            {
                return Err("Invalid retained MCP source scope".into());
            }
            super::native_read::validate_entry(&self.native_name, &entry, &binding.tool)?;
            if connection::read(&entry, &binding.tool)? != self.connection {
                return Err("Invalid retained MCP connection".into());
            }
        }
        let disabled = entry.field("enabled")? == Some(serde_json::Value::Bool(false))
            || entry.field("disabled")? == Some(serde_json::Value::Bool(true));
        if disabled != self.disabled {
            return Err("Invalid retained MCP enabled state".into());
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct SourceSnapshot {
    pub documents: Vec<SourceDocument>,
    pub origins: Vec<NativeOrigin>,
    // Plugin enumeration is a requested scope even when no documents exist.
    // Reconciliation must not infer completeness from just the returned rows.
    pub plugin_roots: Vec<PathBuf>,
}

fn logical_id(path: &std::path::Path, container: &str, name: &str) -> Result<String, String> {
    let path = path
        .to_str()
        .ok_or("MCP source location must use valid Unicode")?;
    let mut digest = Sha256::new();
    digest.update(b"cchub-mcp-origin-v1");
    // Length framing prevents separator characters in names/paths colliding.
    for field in [path, container, name] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field.as_bytes());
    }
    Ok(format!("mcp-origin-{:x}", digest.finalize()))
}

impl SourceSnapshot {
    /// Resolve every configured scope before reading. No database or native writes.
    /// Callers must reconcile the returned complete snapshot in their own group.
    pub fn read_configured(conn: &Connection) -> Result<Self, String> {
        let (bindings, plugin_roots) = scopes::configured(conn)?;
        let mut snapshot = Self::read_bindings(&bindings)?;
        snapshot.plugin_roots = plugin_roots;
        snapshot.verify()?;
        Ok(snapshot)
    }

    /// Recheck requested paths, revisions and plugin membership before catalog
    /// finalization. Equal-byte file replacement identity is a separate gate.
    pub fn verify(&self) -> Result<(), String> {
        for document in &self.documents {
            for binding in &document.bindings {
                if crate::config_write::target_key(&binding.path)? != document.canonical_path
                    || crate::config_write::read(&binding.path)? != document.original
                {
                    return Err("MCP source changed while reading; scan again".into());
                }
            }
        }
        if !self.plugin_roots.is_empty() {
            let mut actual = std::collections::BTreeSet::new();
            for root in &self.plugin_roots {
                let mut files = Vec::new();
                scopes::plugins(root, &mut files)?;
                for file in files {
                    actual.insert(crate::config_write::target_key(&file)?);
                }
            }
            let expected = self
                .documents
                .iter()
                .filter(|document| {
                    document
                        .bindings
                        .iter()
                        .any(|binding| binding.role == SourceRole::Plugin)
                })
                .map(|document| document.canonical_path.clone())
                .collect::<std::collections::BTreeSet<_>>();
            if expected != actual {
                return Err("MCP plugin sources changed while reading; scan again".into());
            }
        }
        for origin in &self.origins {
            if origin.id
                != logical_id(
                    &origin.canonical_path,
                    &origin.container,
                    &origin.native_name,
                )?
            {
                return Err("Invalid MCP source identity".into());
            }
        }
        Ok(())
    }

    /// Read an explicit scope set. Any malformed sibling fails the whole result.
    pub fn read_bindings(bindings: &[SourceBinding]) -> Result<Self, String> {
        let mut documents: BTreeMap<PathBuf, SourceDocument> = BTreeMap::new();
        let mut origins: BTreeMap<String, NativeOrigin> = BTreeMap::new();
        for binding in bindings {
            let canonical = crate::config_write::target_key(&binding.path)?;
            let original = crate::config_write::read(&binding.path)?;
            if crate::config_write::target_key(&binding.path)? != canonical {
                return Err("MCP source location changed while reading; scan again".into());
            }
            if let Some(previous) = documents.get(&canonical) {
                if previous.original != original {
                    return Err("MCP source changed while reading; scan again".into());
                }
            }
            let text = original
                .as_deref()
                .map(std::str::from_utf8)
                .transpose()
                .map_err(|_| "MCP configuration must use UTF-8")?;
            // A missing source is still a verified scope with an absent revision.
            let empty = if matches!(binding.tool.as_str(), "codex" | "grokbuild") {
                ""
            } else {
                "{}"
            };
            let (container, entries) = super::native_read::parse_entries(
                text.unwrap_or(empty),
                &binding.tool,
                binding.role == SourceRole::Plugin,
            )?;
            let document = documents
                .entry(canonical.clone())
                .or_insert_with(|| SourceDocument {
                    canonical_path: canonical.clone(),
                    original,
                    bindings: Vec::new(),
                });
            if !document.bindings.contains(binding) {
                document.bindings.push(binding.clone());
            }
            for (name, entry) in entries {
                let id = logical_id(&canonical, &container, &name)?;
                let connection = connection::read(&entry, &binding.tool)?;
                let disabled = entry.field("enabled")? == Some(serde_json::Value::Bool(false))
                    || entry.field("disabled")? == Some(serde_json::Value::Bool(true));
                let spec = NativeSpec::from_entry(&entry)?;
                if let Some(previous) = origins.get_mut(&id) {
                    if previous.connection != connection
                        || previous.disabled != disabled
                        || previous.spec != spec
                    {
                        return Err("MCP aliases interpret one native entry differently; separate their configured sources".into());
                    }
                    if !previous.bindings.contains(binding) {
                        previous.bindings.push(binding.clone());
                    }
                } else {
                    origins.insert(
                        id.clone(),
                        NativeOrigin {
                            id,
                            native_name: name,
                            canonical_path: canonical.clone(),
                            container: container.clone(),
                            bindings: vec![binding.clone()],
                            connection,
                            disabled,
                            spec,
                        },
                    );
                }
            }
        }
        for document in documents.values_mut() {
            document.bindings.sort();
        }
        for origin in origins.values_mut() {
            origin.bindings.sort();
        }
        let snapshot = Self {
            documents: documents.into_values().collect(),
            origins: origins.into_values().collect(),
            plugin_roots: Vec::new(),
        };
        snapshot.verify()?;
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests;
