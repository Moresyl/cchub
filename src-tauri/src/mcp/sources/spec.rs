use super::super::native_read::Entry;

/// Native encoding for persistence. Never coerce non-finite values/tags to JSON.
/// Exact bytes/comments belong to SourceDocument, not this semantic encoding.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NativeSpec {
    Json(String),
    Toml(String),
    Yaml(String),
}

impl NativeSpec {
    pub(in crate::mcp) fn validate_format(&self, tool: &str) -> Result<(), String> {
        use super::super::native_read::Format;
        let valid = matches!(
            (self, Format::for_tool(tool)?),
            (Self::Toml(_), Format::Codex | Format::Grok)
                | (Self::Yaml(_), Format::Hermes)
                | (
                    Self::Json(_),
                    Format::Standard | Format::Gemini | Format::OpenCode
                )
        );
        if valid {
            Ok(())
        } else {
            Err("Invalid retained MCP native format".into())
        }
    }

    pub(in crate::mcp) fn from_entry(entry: &Entry) -> Result<Self, String> {
        let invalid = || "Cannot retain the native MCP definition".to_owned();
        match entry {
            Entry::Json(fields) => serde_json::to_string(fields)
                .map(Self::Json)
                .map_err(|_| invalid()),
            Entry::Toml(fields) => toml::to_string(fields)
                .map(Self::Toml)
                .map_err(|_| invalid()),
            Entry::Yaml(fields) => serde_yaml::to_string(fields)
                .map(Self::Yaml)
                .map_err(|_| invalid()),
        }
    }

    pub fn to_json(&self) -> Result<serde_json::Value, String> {
        self.entry()?.to_json()
    }

    pub(in crate::mcp) fn entry(&self) -> Result<Entry, String> {
        let invalid = || "Cannot read the retained native MCP definition".to_owned();
        let entry = match self {
            Self::Json(text) => Entry::Json(serde_json::from_str(text).map_err(|_| invalid())?),
            Self::Toml(text) => Entry::Toml(toml::from_str(text).map_err(|_| invalid())?),
            Self::Yaml(text) => Entry::Yaml(serde_yaml::from_str(text).map_err(|_| invalid())?),
        };
        Ok(entry)
    }

    pub(in crate::mcp) fn same(&self, other: &Self) -> Result<bool, String> {
        Ok(match (self.entry()?, other.entry()?) {
            (Entry::Json(left), Entry::Json(right)) => left == right,
            (Entry::Toml(left), Entry::Toml(right)) => {
                crate::mcp::native_toml::same_table(&left, &right)
            }
            (Entry::Yaml(left), Entry::Yaml(right)) => crate::yaml_config::same(
                &serde_yaml::Value::Mapping(left),
                &serde_yaml::Value::Mapping(right),
            ),
            _ => false,
        })
    }
}
