/// Check npm registry for latest version of a package
pub async fn check_npm_version(package_name: &str) -> Result<String, String> {
    check_npm_version_with_proxy(package_name, "").await
}

/// Check npm registry for latest version with proxy support and China mirror fallback
async fn check_npm_version_with_proxy(
    package_name: &str,
    proxy_url: &str,
) -> Result<String, String> {
    let client = if !proxy_url.is_empty() {
        let proxy = reqwest::Proxy::all(proxy_url).map_err(|e| format!("Invalid proxy: {}", e))?;
        reqwest::Client::builder()
            .proxy(proxy)
            .build()
            .map_err(|e| format!("Client build failed: {}", e))?
    } else {
        reqwest::Client::new()
    };

    let registries = [
        format!("https://registry.npmjs.org/{}/latest", package_name),
        format!("https://registry.npmmirror.com/{}/latest", package_name),
    ];

    let mut last_err = String::new();
    for url in &registries {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(data) => {
                        if let Some(ver) = data.get("version").and_then(|v| v.as_str()) {
                            return Ok(ver.to_string());
                        }
                        last_err = "Version field not found".to_string();
                    }
                    Err(e) => {
                        last_err = format!("Parse failed: {}", e);
                    }
                }
            }
            Ok(resp) => {
                last_err = format!("HTTP {}", resp.status());
            }
            Err(e) => {
                last_err = format!("Request failed: {}", e);
            }
        }
    }
    Err(format!("All registries failed: {}", last_err))
}
