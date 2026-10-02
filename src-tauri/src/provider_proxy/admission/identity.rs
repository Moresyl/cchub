use super::super::{managed_auth::AuthProvider, UpstreamTarget};
use axum::http::HeaderMap;
use sha2::{Digest, Sha256};

const CREDENTIAL_HEADERS: [&str; 4] = ["authorization", "x-api-key", "x-goog-api-key", "api-key"];

pub(in crate::provider_proxy) fn uses_managed_identity(
    upstream: &UpstreamTarget,
    headers: &HeaderMap,
    url: &url::Url,
) -> bool {
    let credentials: Vec<_> = upstream
        .headers
        .iter()
        .filter(|(name, _)| {
            CREDENTIAL_HEADERS.contains(&name.to_ascii_lowercase().as_str())
                || name.eq_ignore_ascii_case("chatgpt-account-id")
        })
        .collect();
    upstream.managed_principal.is_some()
        && !credentials.is_empty()
        && !url
            .query_pairs()
            .any(|(name, _)| ["key", "api_key", "access_token", "token"].contains(&name.as_ref()))
        && headers.keys().all(|name| {
            (!CREDENTIAL_HEADERS.contains(&name.as_str()) && name != "chatgpt-account-id")
                || credentials
                    .iter()
                    .any(|(expected, _)| expected.eq_ignore_ascii_case(name.as_str()))
        })
        && credentials.iter().all(|(name, value)| {
            let mut actual = headers.get_all(name.as_str()).iter();
            actual
                .next()
                .is_some_and(|header| header.as_bytes() == value.as_bytes())
                && actual.next().is_none()
        })
}

pub(in crate::provider_proxy) fn scope_key(
    upstream: &UpstreamTarget,
    headers: &HeaderMap,
    url: &url::Url,
) -> Result<String, &'static str> {
    let mut hash = Sha256::new();
    let mut add = |value: &[u8]| {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value);
    };
    add(b"cchub-account-admission-v1");
    if let Some(principal) = &upstream.managed_principal {
        // Credential overrides use API identity; token refresh keeps a stable managed identity.
        if uses_managed_identity(upstream, headers, url) {
            add(b"managed");
            add(match principal.provider {
                AuthProvider::Codex => b"codex",
                AuthProvider::Copilot => b"copilot",
                AuthProvider::Xai => b"xai",
            });
            add(principal.account_id.as_bytes());
            return Ok(format!("{:x}", hash.finalize()));
        }
    }
    // Same service/key across profiles and tools shares one lane. Alternative
    // endpoints share the primary service scope rather than creating new slots.
    let service =
        url::Url::parse(&upstream.base_url).map_err(|_| "Invalid upstream service URL")?;
    add(b"api");
    add(service.origin().ascii_serialization().as_bytes());
    let mut credentials = Vec::new();
    for name in CREDENTIAL_HEADERS {
        for value in headers.get_all(name) {
            let value = value.as_bytes();
            let value = if name == "authorization"
                && value
                    .get(..7)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"bearer "))
            {
                &value[7..]
            } else {
                value
            };
            credentials.push(value.to_vec());
        }
    }
    for (name, value) in url.query_pairs() {
        if ["key", "api_key", "access_token", "token"].contains(&name.as_ref()) {
            credentials.push(value.as_bytes().to_vec());
        }
    }
    credentials.sort();
    credentials.dedup();
    for value in credentials {
        add(&value);
    }
    for value in headers.get_all("chatgpt-account-id") {
        add(value.as_bytes());
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[cfg(test)]
mod tests;
