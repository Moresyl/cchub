use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
}

pub async fn fetch_latest_release(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
) -> Result<GithubRelease, String> {
    let mut last_error = String::new();
    for url in crate::shared::github_urls::latest_release_api_urls(owner, repo) {
        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                return response.json::<GithubRelease>().await.map_err(|error| {
                    format!("Failed to parse GitHub release from {url}: {error}")
                });
            }
            Ok(response) => {
                last_error = format!("HTTP {} from {url}", response.status());
            }
            Err(error) => {
                last_error = format!("Request failed for {url}: {error}");
            }
        }
    }

    Err(format!(
        "Failed to fetch latest release from GitHub Releases API: {last_error}"
    ))
}
