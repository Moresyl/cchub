use super::*;

impl CopilotAuthManager {
    pub(super) fn persist_accounts(
        &self,
        accounts: &HashMap<String, GitHubAccountData>,
        default_account_id: Option<String>,
    ) -> Result<(), CopilotAuthError> {
        let content = serde_json::to_string_pretty(&CopilotAuthStore {
            version: 1,
            accounts: accounts.clone(),
            default_account_id,
        })
        .map_err(|_| CopilotAuthError::Parse("Failed to serialize Copilot account data".into()))?;
        #[cfg(unix)]
        if let Some(parent) = self.storage_path.parent() {
            use std::os::unix::fs::PermissionsExt;
            fs::create_dir_all(parent)?;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        }
        crate::utils::atomic_write_string(&self.storage_path, &content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
