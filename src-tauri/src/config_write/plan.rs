use super::{read, FileUpdate};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(crate) struct FilePlan {
    pub updates: Vec<FileUpdate>,
    pub guards: Vec<(PathBuf, Option<Vec<u8>>)>,
}

impl FilePlan {
    pub(crate) fn extend(&mut self, other: Self) {
        self.updates.extend(other.updates);
        self.guards.extend(other.guards);
    }

    pub(crate) fn replace(&mut self, path: PathBuf, desired: Vec<u8>) -> Result<(), String> {
        self.updates.push(FileUpdate {
            original: read(&path)?,
            path,
            desired,
        });
        Ok(())
    }

    fn check_guards(&self) -> Result<(), String> {
        for (path, original) in &self.guards {
            if &read(path)? != original {
                return Err("Configuration changed externally; reload it and try again".into());
            }
        }
        Ok(())
    }

    pub(crate) fn commit(self) -> Result<(), String> {
        self.commit_then(|| Ok(()))
    }

    pub(crate) fn commit_then(
        self,
        finalize: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        check_distinct(
            self.updates
                .iter()
                .map(|update| update.path.as_path())
                .chain(self.guards.iter().map(|(path, _)| path.as_path())),
        )?;
        self.check_guards()?;
        let Self { updates, guards } = self;
        super::commit_then(updates, || {
            for (path, original) in guards {
                if read(&path)? != original {
                    return Err(
                        "Configuration changed while saving; reload it and try again".into(),
                    );
                }
            }
            finalize()
        })
    }
}

// Existing ancestors can be user-configured links. Canonicalize them so two
// spellings cannot schedule conflicting writes to the same native location.
fn location(path: &Path) -> Result<PathBuf, String> {
    let mut ancestor = std::path::absolute(path).map_err(|_| "Invalid configuration location")?;
    let mut remaining = Vec::new();
    let mut resolved = loop {
        match std::fs::canonicalize(&ancestor) {
            Ok(path) => break path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                remaining.push(
                    ancestor
                        .file_name()
                        .ok_or("Invalid configuration location")?
                        .to_owned(),
                );
                if !ancestor.pop() {
                    return Err("Invalid configuration location".into());
                }
            }
            Err(_) => return Err("Cannot resolve configuration location".into()),
        }
    };
    for name in remaining.into_iter().rev() {
        resolved.push(name);
    }
    #[cfg(windows)]
    {
        resolved = PathBuf::from(
            resolved
                .to_str()
                .ok_or("Invalid configuration location")?
                .to_lowercase(),
        );
    }
    Ok(resolved)
}

pub(super) fn check_distinct<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Result<(), String> {
    let mut locations = std::collections::HashSet::new();
    for path in paths {
        if !locations.insert(location(path)?) {
            return Err("Configuration save targets refer to the same location".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
