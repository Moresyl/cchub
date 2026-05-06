pub fn raw_file_urls(owner: &str, repo: &str, branch: &str, path: &str) -> Vec<String> {
    let normalized_path = path.trim_start_matches('/');
    let canonical =
        format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{normalized_path}");
    with_ghgo_mirror(canonical)
}

pub fn latest_release_api_urls(owner: &str, repo: &str) -> Vec<String> {
    api_urls(owner, repo, "releases/latest", None)
}

pub fn contents_api_urls(owner: &str, repo: &str, branch: &str, path: &str) -> Vec<String> {
    let normalized_path = path.trim_start_matches('/');
    let endpoint = if normalized_path.is_empty() {
        "contents".to_string()
    } else {
        format!("contents/{normalized_path}")
    };
    api_urls(owner, repo, &endpoint, Some(&format!("ref={branch}")))
}

pub fn archive_branch_tarball_urls(owner: &str, repo: &str, branch: &str) -> Vec<String> {
    archive_tarball_urls(owner, repo, "heads", branch)
}

pub fn archive_tag_tarball_urls(owner: &str, repo: &str, tag: &str) -> Vec<String> {
    archive_tarball_urls(owner, repo, "tags", tag)
}

pub fn archive_branch_zip_urls(owner: &str, repo: &str, branch: &str) -> Vec<String> {
    let canonical = format!("https://github.com/{owner}/{repo}/archive/refs/heads/{branch}.zip");
    with_ghgo_mirror(canonical)
}

fn archive_tarball_urls(owner: &str, repo: &str, ref_kind: &str, ref_name: &str) -> Vec<String> {
    let canonical =
        format!("https://github.com/{owner}/{repo}/archive/refs/{ref_kind}/{ref_name}.tar.gz");
    with_ghgo_mirror(canonical)
}

fn api_urls(owner: &str, repo: &str, endpoint: &str, query: Option<&str>) -> Vec<String> {
    let normalized_endpoint = endpoint.trim_start_matches('/');
    let mut canonical =
        format!("https://api.github.com/repos/{owner}/{repo}/{normalized_endpoint}");
    if let Some(query) = query.filter(|value| !value.is_empty()) {
        canonical.push('?');
        canonical.push_str(query);
    }
    with_ghgo_mirror(canonical)
}

fn with_ghgo_mirror(canonical: String) -> Vec<String> {
    let mirror_path = canonical
        .strip_prefix("https://")
        .or_else(|| canonical.strip_prefix("http://"))
        .unwrap_or(&canonical);
    let mirror = format!("https://ghgo.xyz/{mirror_path}");
    vec![canonical, mirror]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_file_urls_include_canonical_before_mirror() {
        let urls = raw_file_urls("owner", "repo", "main", "/dir/file.json");

        assert_eq!(
            urls,
            vec![
                "https://raw.githubusercontent.com/owner/repo/main/dir/file.json",
                "https://ghgo.xyz/raw.githubusercontent.com/owner/repo/main/dir/file.json",
            ]
        );
    }

    #[test]
    fn tag_tarball_urls_include_canonical_before_mirror() {
        let urls = archive_tag_tarball_urls("owner", "repo", "v1.2.3");

        assert_eq!(
            urls,
            vec![
                "https://github.com/owner/repo/archive/refs/tags/v1.2.3.tar.gz",
                "https://ghgo.xyz/github.com/owner/repo/archive/refs/tags/v1.2.3.tar.gz",
            ]
        );
    }

    #[test]
    fn branch_zip_urls_include_canonical_before_mirror() {
        let urls = archive_branch_zip_urls("owner", "repo", "main");

        assert_eq!(
            urls,
            vec![
                "https://github.com/owner/repo/archive/refs/heads/main.zip",
                "https://ghgo.xyz/github.com/owner/repo/archive/refs/heads/main.zip",
            ]
        );
    }

    #[test]
    fn contents_api_urls_preserve_branch_query() {
        let urls = contents_api_urls("owner", "repo", "main", "skills/demo");

        assert_eq!(
            urls,
            vec![
                "https://api.github.com/repos/owner/repo/contents/skills/demo?ref=main",
                "https://ghgo.xyz/api.github.com/repos/owner/repo/contents/skills/demo?ref=main",
            ]
        );
    }
}
