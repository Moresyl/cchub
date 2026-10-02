use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    OptimizerConfig, RectifierConfig, OPTIMIZER_CONFIG_SETTINGS_KEY, RECTIFIER_CONFIG_SETTINGS_KEY,
};

const READ_ERROR: &str = "Cannot read saved proxy settings";
const SAVE_ERROR: &str = "Cannot save proxy settings; the saved settings were preserved";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyAdvancedSettings {
    pub config: OptimizerConfig,
    pub rectifier_config: RectifierConfig,
    pub revision: String,
}

type RawSettings = (Option<String>, Option<String>);

fn read_raw(conn: &Connection) -> Result<RawSettings, String> {
    // Both values come from one SQLite snapshot, including when another connection writes.
    conn.query_row(
        "SELECT (SELECT value FROM app_settings WHERE key = ?1), (SELECT value FROM app_settings WHERE key = ?2)",
        [OPTIMIZER_CONFIG_SETTINGS_KEY, RECTIFIER_CONFIG_SETTINGS_KEY],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|_| READ_ERROR.to_string())
}

fn revision(raw: &RawSettings) -> String {
    let mut hash = Sha256::new();
    hash.update(b"cchub-proxy-settings-v1");
    for value in [&raw.0, &raw.1] {
        hash.update([u8::from(value.is_some())]);
        if let Some(value) = value {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
    }
    format!("{:x}", hash.finalize())
}

fn optimizer(raw: Option<&str>) -> Result<OptimizerConfig, String> {
    let config: OptimizerConfig = match raw {
        None => OptimizerConfig::default(),
        Some(value) => serde_json::from_str(value).map_err(|_| READ_ERROR.to_string())?,
    };
    config
        .validate_timeouts()
        .map_err(|_| READ_ERROR.to_string())?;
    Ok(config)
}

fn rectifier(raw: Option<&str>) -> Result<RectifierConfig, String> {
    match raw {
        None => Ok(RectifierConfig::default()),
        Some(value) => serde_json::from_str(value).map_err(|_| READ_ERROR.to_string()),
    }
}

pub(super) fn read_optimizer(conn: &Connection) -> Result<OptimizerConfig, String> {
    optimizer(read_raw(conn)?.0.as_deref())
}

pub(super) fn read_rectifier(conn: &Connection) -> Result<RectifierConfig, String> {
    rectifier(read_raw(conn)?.1.as_deref())
}

pub(super) fn read(conn: &Connection) -> Result<ProxyAdvancedSettings, String> {
    let raw = read_raw(conn)?;
    Ok(ProxyAdvancedSettings {
        config: optimizer(raw.0.as_deref())?,
        rectifier_config: rectifier(raw.1.as_deref())?,
        revision: revision(&raw),
    })
}

fn serialize_preserving_fields<T: Serialize>(
    config: &T,
    previous: Option<&str>,
) -> Result<String, String> {
    let incoming = serde_json::to_value(config).map_err(|_| SAVE_ERROR.to_string())?;
    let mut saved = match previous {
        Some(value) => {
            serde_json::from_str::<serde_json::Value>(value).map_err(|_| READ_ERROR.to_string())?
        }
        None => serde_json::json!({}),
    };
    let fields = saved.as_object_mut().ok_or(READ_ERROR)?;
    fields.extend(incoming.as_object().ok_or(SAVE_ERROR)?.clone());
    serde_json::to_string(&saved).map_err(|_| SAVE_ERROR.to_string())
}

pub(super) fn save(
    conn: &mut Connection,
    config: OptimizerConfig,
    rectifier_config: RectifierConfig,
    expected_revision: &str,
) -> Result<ProxyAdvancedSettings, String> {
    config.validate_timeouts()?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| SAVE_ERROR.to_string())?;
    let current = read_raw(&tx)?;
    // Reject corrupt settings and stale editors before writing either value.
    optimizer(current.0.as_deref())?;
    rectifier(current.1.as_deref())?;
    if revision(&current) != expected_revision {
        return Err("Proxy settings changed; reload them before saving".into());
    }
    let desired = (
        Some(serialize_preserving_fields(&config, current.0.as_deref())?),
        Some(serialize_preserving_fields(
            &rectifier_config,
            current.1.as_deref(),
        )?),
    );
    for (key, value) in [
        (OPTIMIZER_CONFIG_SETTINGS_KEY, &desired.0),
        (RECTIFIER_CONFIG_SETTINGS_KEY, &desired.1),
    ] {
        let changed = tx
            .execute(
                "INSERT OR REPLACE INTO app_settings(key, value) VALUES (?1, ?2)",
                rusqlite::params![key, value],
            )
            .map_err(|_| SAVE_ERROR.to_string())?;
        if changed != 1 {
            return Err(SAVE_ERROR.into());
        }
    }
    if read_raw(&tx)? != desired {
        return Err(SAVE_ERROR.into());
    }
    tx.commit().map_err(|_| SAVE_ERROR.to_string())?;
    Ok(ProxyAdvancedSettings {
        config,
        rectifier_config,
        revision: revision(&desired),
    })
}

#[cfg(test)]
mod tests;
