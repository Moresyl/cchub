pub(super) fn rewrite_full_url(base_url: &str, query: Option<&str>) -> Result<String, String> {
    let mut url = url::Url::parse(base_url)
        .map_err(|error| format!("Codex Alpha Search base URL is invalid: {error}"))?;
    let path = url.path().trim_end_matches('/');
    if let Some(prefix) = path.strip_suffix("/responses") {
        url.set_path(&format!("{prefix}/alpha/search"));
    } else if !path.ends_with("/alpha/search") {
        return Err(
            "Codex Alpha Search cannot derive /alpha/search from an opaque full URL; use a base URL or a full URL ending in /responses"
                .to_string(),
        );
    }
    if let Some(query) = query.filter(|value| !value.is_empty()) {
        url.set_query(Some(query));
    }
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::rewrite_full_url;

    #[test]
    fn rewrites_known_full_responses_url() {
        let url = rewrite_full_url(
            "https://relay.example/backend-api/codex/responses",
            Some("client_version=0.144.6"),
        )
        .expect("rewrite");
        assert_eq!(
            url,
            "https://relay.example/backend-api/codex/alpha/search?client_version=0.144.6"
        );
    }

    #[test]
    fn rejects_opaque_full_url() {
        let error = rewrite_full_url("https://relay.example/custom/endpoint", None)
            .expect_err("opaque URL must fail closed");
        assert!(error.contains("cannot derive /alpha/search"));
    }
}
