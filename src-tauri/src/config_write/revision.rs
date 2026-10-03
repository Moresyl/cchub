//! A retained handle prevents file-ID reuse while a native revision is pending.
//! Byte guards remain necessary for in-place edits. This is not a filesystem
//! transaction or a guarantee against arbitrary concurrent external writers.
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Identity {
    volume: u64,
    file: u64,
}

fn invalid() -> String {
    "Configuration file identity changed or cannot be verified; reload before continuing".into()
}

#[cfg(windows)]
fn identity(file: &File) -> Result<Identity, String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // The borrowed File keeps its handle live; the API initializes the entire
    // output structure only on success, which is checked before assume_init.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
        return Err(invalid());
    }
    let info = unsafe { info.assume_init() };
    Ok(Identity {
        volume: u64::from(info.dwVolumeSerialNumber),
        file: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    })
}

#[cfg(unix)]
fn identity(file: &File) -> Result<Identity, String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata().map_err(|_| invalid())?;
    Ok(Identity {
        volume: metadata.dev(),
        file: metadata.ino(),
    })
}

fn open(path: &Path, directory: bool) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    if directory {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES,
        };
        options
            .access_mode(FILE_READ_ATTRIBUTES)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
    }
    #[cfg(not(windows))]
    let _ = directory;
    options.open(path)
}

#[derive(Clone, Debug)]
struct Pin {
    path: PathBuf,
    handle: Arc<File>,
    id: Identity,
    directory: bool,
}

impl Pin {
    fn new(path: &Path, file: File, directory: bool) -> Result<Self, String> {
        let metadata = file.metadata().map_err(|_| invalid())?;
        if metadata.is_dir() != directory || (!directory && !metadata.is_file()) {
            return Err(invalid());
        }
        Ok(Self {
            path: path.into(),
            id: identity(&file)?,
            handle: Arc::new(file),
            directory,
        })
    }

    fn verify(&self) -> Result<(), String> {
        let current = open(&self.path, self.directory).map_err(|_| invalid())?;
        if identity(&current)? != self.id || identity(&self.handle)? != self.id {
            return Err(invalid());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FileRevision {
    path: PathBuf,
    target: Option<Pin>,
    ancestor: Pin,
}

impl FileRevision {
    pub(crate) fn capture(path: &Path) -> Result<(Self, Option<Vec<u8>>), String> {
        let canonical = super::target_key(path)?;
        let target = match open(&canonical, false) {
            Ok(mut file) => {
                let before = file.metadata().map_err(|_| invalid())?;
                if !before.is_file() {
                    return Err(invalid());
                }
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).map_err(|_| invalid())?;
                let after = file.metadata().map_err(|_| invalid())?;
                if before.len() != after.len()
                    || before.modified().map_err(|_| invalid())?
                        != after.modified().map_err(|_| invalid())?
                {
                    return Err(invalid());
                }
                Some((Pin::new(&canonical, file, false)?, bytes))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(invalid()),
        };
        let mut parent = canonical.parent().ok_or_else(invalid)?;
        let ancestor = loop {
            match open(parent, true) {
                Ok(file) => break Pin::new(parent, file, true)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    parent = parent.parent().ok_or_else(invalid)?;
                }
                Err(_) => return Err(invalid()),
            }
        };
        let (target, bytes) = match target {
            Some((pin, bytes)) => (Some(pin), Some(bytes)),
            None => (None, None),
        };
        let revision = Self {
            path: canonical.clone(),
            target,
            ancestor,
        };
        revision.verify()?;
        if super::target_key(path)? != canonical {
            return Err(invalid());
        }
        Ok((revision, bytes))
    }

    pub(crate) fn verify_parents(&self) -> Result<(), String> {
        if super::target_key(&self.path)? != self.path {
            return Err(invalid());
        }
        self.ancestor.verify()
    }

    pub(crate) fn verify(&self) -> Result<(), String> {
        self.verify_parents()?;
        match &self.target {
            Some(target) => target.verify(),
            None => match std::fs::symlink_metadata(&self.path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(invalid()),
            },
        }
    }
}

#[cfg(test)]
mod tests;
