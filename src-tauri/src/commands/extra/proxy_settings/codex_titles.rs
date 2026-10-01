use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::value::RawValue;

use super::super::config_profiles::count_query_hits;
use super::super::types::SessionSummary;

const MAX_INDEX_LINE: usize = 1024 * 1024;
const MAX_TITLE_CHARS: usize = 200;

#[derive(Deserialize)]
struct Name<'a> {
    id: String,
    #[serde(borrow)]
    thread_name: Option<&'a RawValue>,
}

// Only retain names requested by this scan. Reading the current index avoids
// stale titles after same-size rewrites or file replacement, and never shares
// a cache between different configured installations or project directories.
fn load(root: &Path, requested: &HashSet<&str>) -> HashMap<String, String> {
    let path = root.join("session_index.jsonl");
    let Ok(metadata) = std::fs::symlink_metadata(&path) else {
        return HashMap::new();
    };
    if !metadata.is_file() || requested.is_empty() {
        return HashMap::new();
    }
    let Ok(file) = File::open(path) else {
        return HashMap::new();
    };
    read_names(BufReader::new(file), requested).unwrap_or_default()
}

fn read_names(
    mut reader: impl BufRead,
    requested: &HashSet<&str>,
) -> std::io::Result<HashMap<String, String>> {
    let mut names = HashMap::new();
    let mut line = Vec::with_capacity(4096);
    loop {
        line.clear();
        let count = (&mut reader)
            .take((MAX_INDEX_LINE + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if count == 0 {
            break;
        }
        if line.len() > MAX_INDEX_LINE {
            // A bad/huge line must not allocate indefinitely or hide valid
            // renames that follow it in the append-only index.
            if line.last() != Some(&b'\n') {
                reader.skip_until(b'\n')?;
            }
            continue;
        }
        let Ok(row) = serde_json::from_slice::<Name<'_>>(&line) else {
            continue;
        };
        if row.id.len() > 512
            || row.id.chars().any(char::is_control)
            || !requested.contains(row.id.as_str())
        {
            continue;
        }
        let Some(raw) = row.thread_name else {
            continue;
        };
        let Ok(name) = serde_json::from_str::<String>(raw.get()) else {
            continue;
        };
        let normalized = name.split_whitespace().collect::<Vec<_>>().join(" ");
        let name = normalized
            .chars()
            .filter(|character| !character.is_control())
            .take(MAX_TITLE_CHARS)
            .collect::<String>();
        if !name.is_empty() {
            names.insert(row.id, name);
        }
    }
    Ok(names)
}

pub(super) fn finish(
    root: &Path,
    generic_roots: &[PathBuf],
    mut sessions: Vec<SessionSummary>,
    first_messages: &HashMap<String, String>,
    query: &str,
) -> Vec<SessionSummary> {
    let mut owners = HashMap::<PathBuf, Vec<usize>>::new();
    let roots = generic_roots
        .iter()
        .filter_map(|path| path.canonicalize().ok())
        .collect::<Vec<_>>();
    for (index, session) in sessions.iter().enumerate() {
        // A native threads database belongs to its configured installation,
        // even when its rollout is stored elsewhere. Generic project scans
        // instead use the deepest explicitly discovered owning directory.
        let owner = if first_messages.contains_key(&session.id) {
            Some(root.to_path_buf())
        } else {
            Path::new(&session.source_path)
                .canonicalize()
                .ok()
                .and_then(|source| {
                    roots
                        .iter()
                        .filter(|candidate| source.starts_with(candidate))
                        .max_by_key(|candidate| candidate.components().count())
                        .cloned()
                })
        };
        if let Some(owner) = owner {
            owners.entry(owner).or_default().push(index);
        }
    }
    for (owner, indices) in owners {
        let requested = indices
            .iter()
            .map(|index| sessions[*index].id.as_str())
            .collect::<HashSet<_>>();
        let names = load(&owner, &requested);
        for index in indices {
            let session = &mut sessions[index];
            if let Some(title) = names.get(&session.id) {
                session.title.clone_from(title);
            }
        }
    }
    sessions.retain_mut(|session| {
        let mut values = vec![
            session.title.clone(),
            session.preview.clone(),
            session.cwd.clone().unwrap_or_default(),
            session.id.clone(),
        ];
        if let Some(first) = first_messages.get(&session.id) {
            values.push(first.clone());
        }
        session.search_hit_count = count_query_hits(query, &values);
        query.is_empty() || session.search_hit_count > 0
    });
    sessions
}

#[cfg(test)]
mod tests;
