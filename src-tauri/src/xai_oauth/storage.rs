use super::*;

impl XaiOAuthManager {
    pub(super) fn load_from_disk_sync(&self) -> Result<(), XaiOAuthError> {
        if !self.storage_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&self.storage_path)?;
        let mut store: Store = serde_json::from_str(&content)
            .map_err(|error| XaiOAuthError::Parse(error.to_string()))?;
        let mut migrated = false;
        for (id, account) in &mut store.accounts {
            if let Some(token) = account.refresh_token.take() {
                if keyring_set(id, &token).is_err() {
                    account.refresh_token = Some(token);
                } else {
                    migrated = true;
                }
            }
        }
        if migrated {
            let content = serde_json::to_string_pretty(&store)
                .map_err(|error| XaiOAuthError::Parse(error.to_string()))?;
            self.write_store_atomic(&content)?;
        }
        if let Ok(mut accounts) = self.accounts.try_write() {
            *accounts = store.accounts;
        }
        if let Ok(mut default) = self.default_account_id.try_write() {
            *default = store.default_account_id;
        }
        Ok(())
    }

    pub(super) async fn save_to_disk(&self) -> Result<(), XaiOAuthError> {
        let store = Store {
            version: 1,
            accounts: self.accounts.read().await.clone(),
            default_account_id: self.resolve_default_account_id().await,
        };
        let content = serde_json::to_string_pretty(&store)
            .map_err(|error| XaiOAuthError::Parse(error.to_string()))?;
        self.write_store_atomic(&content)
    }

    fn write_store_atomic(&self, content: &str) -> Result<(), XaiOAuthError> {
        let parent = self
            .storage_path
            .parent()
            .ok_or_else(|| XaiOAuthError::Io("Invalid OAuth storage path".to_string()))?;
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        }
        crate::utils::atomic_write_string(&self.storage_path, content)?;
        Ok(())
    }
}
