//! Credentials belong to a remote account, never to the currently open form.
use sha2::{Digest, Sha256};

pub(crate) trait CredentialStore {
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    fn delete(&self, account: &str) -> Result<(), String>;
}

pub(crate) struct KeyringStore;

impl CredentialStore for KeyringStore {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        match entry(account)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("无法读取系统凭据，请检查系统密钥环是否可用".into()),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        entry(account)?
            .set_password(secret)
            .map_err(|_| "无法保存系统凭据，请检查系统密钥环是否可用".into())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        match entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("无法删除系统凭据，请检查系统密钥环是否可用".into()),
        }
    }
}

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("cchub", account).map_err(|_| "无法打开系统密钥环".into())
}

pub(crate) fn validate_url(value: &str) -> Result<url::Url, String> {
    let url =
        url::Url::parse(value).map_err(|_| "服务器地址必须是完整的 HTTP(S) URL".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("服务器地址必须使用 HTTP 或 HTTPS".into());
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("服务器地址不能包含账号、密码、查询参数或片段，请在对应字段填写凭据".into());
    }
    Ok(url)
}

pub(crate) fn scope(kind: &str, remote: &str, identity: &str) -> String {
    let canonical = url::Url::parse(remote)
        .map(|url| url.to_string().trim_end_matches('/').to_string())
        .unwrap_or_else(|_| remote.trim().trim_end_matches('/').to_string());
    let mut hash = Sha256::new();
    for part in [canonical.as_str(), identity.trim()] {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    format!("{kind}_{:x}", hash.finalize())
}

/// Preserve an existing entry if persisting its associated settings fails.
pub(crate) fn save(
    store: &impl CredentialStore,
    account: &str,
    secret: &str,
    persist: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let previous = store.get(account)?;
    let next = (!secret.trim().is_empty()).then_some(secret);
    if previous.as_deref() == next {
        return persist();
    }
    match next {
        Some(secret) => store.set(account, secret)?,
        None => store.delete(account)?,
    }
    if let Err(error) = persist() {
        let rollback = match previous {
            Some(secret) => store.set(account, &secret),
            None => store.delete(account),
        };
        return Err(if rollback.is_err() {
            "设置保存失败，且系统凭据未能回退；请重新填写凭据后保存".into()
        } else {
            error
        });
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashMap};

    #[derive(Default)]
    pub(crate) struct MemoryStore(pub RefCell<HashMap<String, String>>);

    impl CredentialStore for MemoryStore {
        fn get(&self, account: &str) -> Result<Option<String>, String> {
            Ok(self.0.borrow().get(account).cloned())
        }
        fn set(&self, account: &str, secret: &str) -> Result<(), String> {
            self.0.borrow_mut().insert(account.into(), secret.into());
            Ok(())
        }
        fn delete(&self, account: &str) -> Result<(), String> {
            self.0.borrow_mut().remove(account);
            Ok(())
        }
    }

    #[test]
    fn scope_canonicalizes_urls_and_separates_accounts() {
        assert_eq!(
            scope("dav", "https://DAV.test:443/a/", " u "),
            scope("dav", "https://dav.test/a", "u")
        );
        assert_ne!(
            scope("dav", "https://dav.test/a", "u"),
            scope("dav", "https://dav.test/b", "u")
        );
        assert_ne!(
            scope("dav", "https://dav.test/a", "u"),
            scope("dav", "https://dav.test/a", "v")
        );
        assert!(!scope("dav", "https://dav.test/a", "user").contains("user"));
    }

    #[test]
    fn remote_url_rejects_embedded_secrets_and_unsigned_queries() {
        for value in [
            "ftp://host/path",
            "https://user:password@host",
            "https://host?token=secret",
            "https://host#fragment",
            "invalid",
        ] {
            let error = validate_url(value).unwrap_err();
            assert!(!error.contains("secret"));
            assert!(!error.contains("password"));
        }
        assert!(validate_url("http://127.0.0.1:9000/storage").is_ok());
    }

    #[test]
    fn failed_database_save_restores_previous_secret_or_absence() {
        let store = MemoryStore::default();
        store.set("existing", "original").unwrap();
        assert!(save(
            &store,
            "existing",
            "replacement",
            || Err("db failed".into())
        )
        .is_err());
        assert_eq!(store.get("existing").unwrap().as_deref(), Some("original"));
        assert!(save(&store, "new", "replacement", || Err("db failed".into())).is_err());
        assert_eq!(store.get("new").unwrap(), None);
        assert!(save(&store, "existing", "", || Err("db failed".into())).is_err());
        assert_eq!(store.get("existing").unwrap().as_deref(), Some("original"));
    }
}
