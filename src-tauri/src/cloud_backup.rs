//! Versioned authenticated backup payloads, independent of remote login secrets.
use std::num::NonZeroU32;

use ring::{
    aead, pbkdf2,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Deserializer, Serialize};
use zeroize::Zeroizing;

use crate::cloud_credentials::CredentialStore;

pub(crate) const PAYLOAD_FORMAT: &str = "cchub-sealed-sql-v1";
pub(crate) const PLAINTEXT_LIMIT: usize = 15 * 1024 * 1024;
const MAGIC: &[u8; 8] = b"CCHUBENC";
const HEADER_LEN: usize = 52;
const TAG_LEN: usize = 16;
const ITERATIONS: u32 = 600_000;
pub(crate) const ENCODED_LIMIT: usize = PLAINTEXT_LIMIT + HEADER_LEN + TAG_LEN;
const INVALID: &str = "加密备份格式无效或版本不受支持，未导入备份";
const AUTH_FAILED: &str = "备份密码错误或备份已被修改，未导入备份";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BackupEncryption {
    #[serde(skip_serializing, deserialize_with = "deserialize_passphrase")]
    pub passphrase: Zeroizing<String>,
    pub has_passphrase: bool,
    #[serde(skip_serializing)]
    pub passphrase_touched: bool,
}

impl Default for BackupEncryption {
    fn default() -> Self {
        Self {
            passphrase: Zeroizing::new(String::new()),
            has_passphrase: false,
            passphrase_touched: false,
        }
    }
}

impl std::fmt::Debug for BackupEncryption {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BackupEncryption")
            .field("has_passphrase", &self.has_passphrase)
            .finish_non_exhaustive()
    }
}

fn deserialize_passphrase<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Zeroizing<String>, D::Error> {
    String::deserialize(deserializer).map(Zeroizing::new)
}

impl BackupEncryption {
    pub(crate) fn load(&mut self, store: &impl CredentialStore, scope: &str) -> Result<(), String> {
        self.passphrase = Zeroizing::new(store.get(scope)?.unwrap_or_default());
        self.has_passphrase = !self.passphrase.is_empty();
        self.passphrase_touched = false;
        Ok(())
    }

    pub(crate) fn prepare_save(
        &mut self,
        store: &impl CredentialStore,
        scope: &str,
        auto_sync: bool,
    ) -> Result<(), String> {
        if !self.passphrase_touched && self.passphrase.is_empty() {
            self.passphrase = Zeroizing::new(store.get(scope)?.unwrap_or_default());
        }
        self.has_passphrase = !self.passphrase.is_empty();
        if self.has_passphrase {
            validate_new_passphrase(&self.passphrase)?;
        }
        if auto_sync && !self.has_passphrase {
            return Err("请先设置备份密码，再启用自动上传".into());
        }
        Ok(())
    }

    pub(crate) fn masked(&self) -> Self {
        Self {
            has_passphrase: self.has_passphrase || !self.passphrase.is_empty(),
            ..Default::default()
        }
    }
}

pub(crate) fn validate_new_passphrase(passphrase: &str) -> Result<(), String> {
    if passphrase.trim().is_empty() || passphrase.chars().count() < 12 || passphrase.len() > 1024 {
        return Err("备份密码至少需要 12 个字符，且不能超过 1024 字节".into());
    }
    Ok(())
}

pub(crate) fn validate_format(format: &str) -> Result<(), String> {
    if format.is_empty() || format == PAYLOAD_FORMAT {
        Ok(())
    } else {
        Err(INVALID.into())
    }
}

pub(crate) fn encrypted(format: &str, path: &str) -> bool {
    format == PAYLOAD_FORMAT || path.ends_with(".cchub-backup")
}

fn key(passphrase: &str, salt: &[u8], iterations: u32) -> Result<aead::LessSafeKey, String> {
    if passphrase.trim().is_empty() || passphrase.len() > 1024 {
        return Err(AUTH_FAILED.into());
    }
    let count = NonZeroU32::new(iterations).ok_or(INVALID)?;
    let mut derived = Zeroizing::new([0u8; 32]);
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        count,
        salt,
        passphrase.as_bytes(),
        &mut *derived,
    );
    aead::UnboundKey::new(&aead::AES_256_GCM, &*derived)
        .map(aead::LessSafeKey::new)
        .map_err(|_| INVALID.into())
}

fn seal(mut plain: Zeroizing<Vec<u8>>, passphrase: Zeroizing<String>) -> Result<Vec<u8>, String> {
    validate_new_passphrase(&passphrase)?;
    if plain.is_empty() || plain.len() > PLAINTEXT_LIMIT {
        return Err("备份内容为空或超过 15 MiB，未上传".into());
    }
    let mut header = [0u8; HEADER_LEN];
    header[..8].copy_from_slice(MAGIC);
    header[8..12].copy_from_slice(&[1, 1, 1, 0]); // Version, AES-256-GCM, PBKDF2-SHA256, reserved.
    header[12..16].copy_from_slice(&ITERATIONS.to_be_bytes());
    let random = SystemRandom::new();
    random
        .fill(&mut header[16..32])
        .map_err(|_| "无法生成安全随机数，未上传备份".to_string())?;
    random
        .fill(&mut header[32..44])
        .map_err(|_| "无法生成安全随机数，未上传备份".to_string())?;
    header[44..52].copy_from_slice(&(plain.len() as u64).to_be_bytes());
    let nonce = aead::Nonce::try_assume_unique_for_key(&header[32..44]).map_err(|_| INVALID)?;
    key(&passphrase, &header[16..32], ITERATIONS)?
        .seal_in_place_append_tag(nonce, aead::Aad::from(&header), &mut *plain)
        .map_err(|_| "加密备份失败，未上传".to_string())?;
    let mut result = Vec::with_capacity(header.len() + plain.len());
    result.extend_from_slice(&header);
    result.extend_from_slice(&plain);
    Ok(result)
}

fn open(bytes: Vec<u8>, passphrase: Zeroizing<String>) -> Result<Zeroizing<Vec<u8>>, String> {
    if bytes.len() < HEADER_LEN + TAG_LEN || bytes.len() > ENCODED_LIMIT {
        return Err(INVALID.into());
    }
    let (header, ciphertext) = bytes.split_at(HEADER_LEN);
    if &header[..8] != MAGIC || header[8..12] != [1, 1, 1, 0] {
        return Err(INVALID.into());
    }
    let iterations = u32::from_be_bytes(header[12..16].try_into().map_err(|_| INVALID)?);
    if !(100_000..=1_000_000).contains(&iterations) {
        return Err(INVALID.into());
    }
    let length = u64::from_be_bytes(header[44..52].try_into().map_err(|_| INVALID)?);
    if length == 0
        || length > PLAINTEXT_LIMIT as u64
        || length + TAG_LEN as u64 != ciphertext.len() as u64
    {
        return Err(INVALID.into());
    }
    let nonce = aead::Nonce::try_assume_unique_for_key(&header[32..44]).map_err(|_| INVALID)?;
    let mut plain = Zeroizing::new(ciphertext.to_vec());
    let decoded_len = key(&passphrase, &header[16..32], iterations)?
        .open_in_place(nonce, aead::Aad::from(header), &mut plain)
        .map_err(|_| AUTH_FAILED)?
        .len();
    plain.truncate(decoded_len);
    Ok(plain)
}

pub(crate) async fn seal_async(
    bytes: Vec<u8>,
    passphrase: Zeroizing<String>,
) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || seal(Zeroizing::new(bytes), passphrase))
        .await
        .map_err(|_| "备份加密任务失败，未上传".to_string())?
}

pub(crate) async fn open_for_restore(
    bytes: Vec<u8>,
    format: String,
    path: String,
    passphrase: Zeroizing<String>,
    allow_plaintext: bool,
) -> Result<Zeroizing<Vec<u8>>, String> {
    validate_format(&format)?;
    if encrypted(&format, &path) || bytes.starts_with(MAGIC) {
        return tokio::task::spawn_blocking(move || open(bytes, passphrase))
            .await
            .map_err(|_| "备份解密任务失败，未导入备份".to_string())?;
    }
    if !allow_plaintext {
        return Err("这是旧的未加密备份，请先明确确认使用旧格式恢复".into());
    }
    if bytes.is_empty() || bytes.len() > PLAINTEXT_LIMIT {
        return Err(INVALID.into());
    }
    Ok(Zeroizing::new(bytes))
}

#[cfg(test)]
mod tests;
