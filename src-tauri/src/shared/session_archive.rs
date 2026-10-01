//! Streaming session archives with stable logical paths and explicit limits.
use std::collections::{HashSet, VecDeque};
use std::fs::{self, File, Metadata};
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Component, Path, PathBuf};

pub(crate) const MAX_SESSION_BYTES: u64 = 256 * 1024 * 1024;
const MAX_LINE_BYTES: u64 = 8 * 1024 * 1024;

pub(crate) fn compressed(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".jsonl.zst"))
}

pub(crate) fn jsonl(path: &Path) -> bool {
    compressed(path)
        || path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
}

pub(crate) fn logical_path(path: &Path) -> PathBuf {
    if compressed(path) {
        path.with_extension("")
    } else {
        path.to_path_buf()
    }
}

pub(crate) fn twin(path: &Path) -> Option<PathBuf> {
    if !jsonl(path) {
        return None;
    }
    if compressed(path) {
        return Some(logical_path(path));
    }
    let mut name = path.as_os_str().to_os_string();
    name.push(".zst");
    Some(PathBuf::from(name))
}

pub(crate) fn is_link(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn regular(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || is_link(&meta) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Session source is not a regular file",
        ));
    }
    Ok(())
}

/// Prefer a plain twin while packing. Only a genuinely missing file falls back.
pub(crate) fn resolve(path: &Path) -> io::Result<PathBuf> {
    let plain = logical_path(path);
    match regular(&plain) {
        Ok(()) => Ok(plain),
        Err(error) if error.kind() == io::ErrorKind::NotFound && jsonl(path) => {
            let packed = twin(&plain).expect("JSONL has a twin");
            regular(&packed)?;
            Ok(packed)
        }
        Err(error) => Err(error),
    }
}

/// The configured root itself may be linked; its descendants must stay owned.
pub(crate) fn confined(path: &Path, root: &Path) -> bool {
    owned_destination(path, root, false)
}

pub(crate) fn owned_destination(path: &Path, root: &Path, allow_missing: bool) -> bool {
    let Ok(canonical_root) = root.canonicalize() else {
        return false;
    };
    if canonical_root.is_file() {
        return path.canonicalize().is_ok_and(|p| p == canonical_root);
    }
    let Ok(relative) = path
        .strip_prefix(root)
        .or_else(|_| path.strip_prefix(&canonical_root))
    else {
        return false;
    };
    let mut current = canonical_root.clone();
    for part in relative.components() {
        match part {
            Component::Normal(name) => current.push(name),
            Component::CurDir => continue,
            _ => return false,
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) if is_link(&meta) => return false,
            Ok(_) => {}
            Err(error) if allow_missing && error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return false,
        }
    }
    if allow_missing {
        current.starts_with(&canonical_root)
    } else {
        current.is_file()
            && current
                .canonicalize()
                .is_ok_and(|p| p.starts_with(&canonical_root))
    }
}

pub(crate) fn preferred_files(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter_map(|path| {
            let path = resolve(&path).ok()?;
            let key = logical_path(&path.canonicalize().ok()?);
            seen.insert(key).then_some(path)
        })
        .collect()
}

struct LimitedRead<R> {
    reader: R,
    remaining: u64,
}
impl<R: Read> Read for LimitedRead<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return match self.reader.read(&mut [0u8; 1])? {
                0 => Ok(0),
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Session exceeds the decoded size limit",
                )),
            };
        }
        let limit = output
            .len()
            .min(self.remaining.min(usize::MAX as u64) as usize);
        let count = self.reader.read(&mut output[..limit])?;
        self.remaining -= count as u64;
        Ok(count)
    }
}

pub(crate) fn open(path: &Path, max_bytes: u64) -> io::Result<Box<dyn Read>> {
    let path = resolve(path)?;
    open_exact(&path, compressed(&path), max_bytes)
}

pub(crate) fn open_exact(path: &Path, packed: bool, max_bytes: u64) -> io::Result<Box<dyn Read>> {
    regular(path)?;
    let file = File::open(path)?;
    let reader: Box<dyn Read> = if packed {
        let mut decoder = zstd::stream::read::Decoder::new(file)?;
        decoder.window_log_max(27)?;
        Box::new(decoder)
    } else {
        Box::new(file)
    };
    Ok(Box::new(LimitedRead {
        reader,
        remaining: max_bytes,
    }))
}

pub(crate) struct SessionLines {
    reader: BufReader<Box<dyn Read>>,
    stopped: bool,
}
impl Iterator for SessionLines {
    type Item = io::Result<String>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None;
        }
        let mut line = Vec::new();
        let result = (&mut self.reader)
            .take(MAX_LINE_BYTES + 1)
            .read_until(b'\n', &mut line)
            .and_then(|count| {
                if count == 0 {
                    return Ok(None);
                }
                if count as u64 > MAX_LINE_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Session line exceeds the size limit",
                    ));
                }
                if line.last() == Some(&b'\n') {
                    line.pop();
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                }
                String::from_utf8(line).map(Some).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "Session contains invalid UTF-8")
                })
            });
        match result {
            Ok(Some(line)) => Some(Ok(line)),
            Ok(None) => {
                self.stopped = true;
                None
            }
            Err(error) => {
                self.stopped = true;
                Some(Err(error))
            }
        }
    }
}

pub(crate) fn lines(path: &Path, max_bytes: u64) -> io::Result<SessionLines> {
    Ok(reader_lines(open(path, max_bytes)?))
}

pub(crate) fn reader_lines(reader: Box<dyn Read>) -> SessionLines {
    SessionLines {
        reader: BufReader::new(reader),
        stopped: false,
    }
}

/// Keep only the decoded tail while validating the entire bounded stream.
pub(crate) fn tail(path: &Path, max_bytes: u64, keep: usize) -> io::Result<Vec<u8>> {
    let mut reader = open(path, max_bytes)?;
    let mut retained = VecDeque::with_capacity(keep);
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            return Ok(retained.into_iter().collect());
        }
        let excess = retained.len().saturating_add(count).saturating_sub(keep);
        retained.drain(..excess.min(retained.len()));
        retained.extend(&chunk[count.saturating_sub(keep)..count]);
    }
}

#[cfg(test)]
mod tests;
