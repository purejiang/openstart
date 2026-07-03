use serde::Serialize;

/// Response from the GitHub Releases API (latest release).
#[derive(Debug, Serialize, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub download_url: String,
    pub release_notes: String,
}

/// Compare two semver strings like "1.2.3". Returns `Ordering`.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.')
            .map(|s| s.parse::<u32>().unwrap_or(0))
            .collect()
    };
    let va = parse(a);
    let vb = parse(b);
    let len = va.len().max(vb.len());
    for i in 0..len {
        let na = va.get(i).copied().unwrap_or(0);
        let nb = vb.get(i).copied().unwrap_or(0);
        match na.cmp(&nb) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

/// Fetch the latest GitHub release and compare with the current version.
pub fn check_for_updates() -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    let resp = ureq::get("https://api.github.com/repos/purejiang/openstart/releases/latest")
        .set("User-Agent", "OpenStart-Updater")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Network error: {}", e))?;

    let json: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let tag = json["tag_name"]
        .as_str()
        .ok_or("Missing 'tag_name' in response")?;
    let latest_version = tag.strip_prefix('v').unwrap_or(tag).to_string();
    let download_url = json["html_url"]
        .as_str()
        .unwrap_or("https://github.com/purejiang/openstart/releases")
        .to_string();
    let release_notes = json["body"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let update_available =
        compare_versions(&latest_version, &current_version) == std::cmp::Ordering::Greater;

    Ok(UpdateInfo {
        current_version,
        latest_version,
        update_available,
        download_url,
        release_notes,
    })
}
