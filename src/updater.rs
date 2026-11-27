use std::future::Future;
use std::pin::Pin;

#[derive(Debug, Clone)]
pub enum UpdateState {
    Idle,
    Checking,
    Available { version: String, url: String },
    UpToDate,
    Error(String),
}

#[derive(Debug, Clone)]
pub enum UpdateResult {
    Available { version: String, url: String },
    UpToDate,
    Error(String),
}

pub fn check_updates_future(
    repo: String,
    current_version: String,
) -> Pin<Box<dyn Future<Output = UpdateResult> + Send>> {
    Box::pin(async move {
        let url = format!("https://api.github.com/repos/{repo}/releases/latest");
        match crate::runtime::spawn_blocking(move || {
            fetch_latest_release_blocking(&url, &current_version, &repo)
        })
        .await
        {
            Ok(result) => result,
            Err(_) => UpdateResult::Error("updater task dropped".to_string()),
        }
    })
}

pub fn fetch_latest_release_blocking(url: &str, current_version: &str, repo: &str) -> UpdateResult {
    let response = match ureq::get(url)
        .set("User-Agent", "IMGEditor")
        .set("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::Status(404, _)) => return UpdateResult::UpToDate,
        Err(error) => return UpdateResult::Error(error.to_string()),
    };

    let body = match response.into_string() {
        Ok(body) => body,
        Err(error) => return UpdateResult::Error(error.to_string()),
    };

    parse_latest_release_response(&body, current_version, repo)
}

fn parse_latest_release_response(body: &str, current_version: &str, repo: &str) -> UpdateResult {
    let value: serde_json::Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(error) => return UpdateResult::Error(error.to_string()),
    };

    let Some(object) = value.as_object() else {
        return UpdateResult::Error("unexpected GitHub response".to_string());
    };

    if object.get("draft").and_then(|v| v.as_bool()) == Some(true)
        || object.get("prerelease").and_then(|v| v.as_bool()) == Some(true)
    {
        return UpdateResult::UpToDate;
    }

    let tag_name = match object.get("tag_name").and_then(|name| name.as_str()) {
        Some(name) => name,
        None => return UpdateResult::Error("missing release tag name".to_string()),
    };

    let latest = match parse_version(tag_name) {
        Some(version) => version,
        None => return UpdateResult::Error(format!("invalid release version: {tag_name}")),
    };

    let current = match parse_version(current_version) {
        Some(version) => version,
        None => return UpdateResult::Error(format!("invalid current version: {current_version}")),
    };

    if latest > current {
        let release_url = object
            .get("html_url")
            .and_then(|u| u.as_str())
            .filter(|u| u.starts_with("https://github.com/"))
            .map(str::to_string)
            .unwrap_or_else(|| format!("https://github.com/{repo}/releases/latest"));
        UpdateResult::Available {
            version: tag_name.to_string(),
            url: release_url,
        }
    } else {
        UpdateResult::UpToDate
    }
}

fn parse_version(value: &str) -> Option<(u32, u32, u32)> {
    let cleaned = value.trim().trim_start_matches('v').trim_start_matches('V');
    let mut parts = cleaned.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_examples() {
        assert_eq!(parse_version("0.8.0"), Some((0, 8, 0)));
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("2.0"), Some((2, 0, 0)));
        assert_eq!(parse_version("not-a-version"), None);
    }

    #[test]
    fn comparison_is_not_lexicographic() {
        assert!(parse_version("0.10.0").unwrap() > parse_version("0.9.0").unwrap());
        assert!(parse_version("1.2.10").unwrap() > parse_version("1.2.2").unwrap());
    }

    #[test]
    fn same_version_is_up_to_date() {
        let current = parse_version("3.3.0").unwrap();
        let latest = parse_version("v3.3.0").unwrap();
        assert!(latest <= current);
    }

    #[test]
    fn latest_release_response_detects_newer_release_and_uses_html_url() {
        let body = r#"{
            "tag_name": "v4.18.0",
            "html_url": "https://github.com/CloudyTabzy/IMGEditor-Plus/releases/tag/v4.18.0",
            "draft": false,
            "prerelease": false
        }"#;
        match parse_latest_release_response(body, "4.16.1", "CloudyTabzy/IMGEditor-Plus") {
            UpdateResult::Available { version, url } => {
                assert_eq!(version, "v4.18.0");
                assert_eq!(
                    url,
                    "https://github.com/CloudyTabzy/IMGEditor-Plus/releases/tag/v4.18.0"
                );
            }
            other => panic!("expected Available, got {other:?}"),
        }
    }

    #[test]
    fn latest_release_response_ignores_same_older_draft_and_prerelease() {
        let same = r#"{"tag_name":"v4.18.0","draft":false,"prerelease":false}"#;
        assert!(matches!(
            parse_latest_release_response(same, "4.18.0", "CloudyTabzy/IMGEditor-Plus"),
            UpdateResult::UpToDate
        ));

        let draft = r#"{"tag_name":"v4.19.0","draft":true,"prerelease":false}"#;
        assert!(matches!(
            parse_latest_release_response(draft, "4.18.0", "CloudyTabzy/IMGEditor-Plus"),
            UpdateResult::UpToDate
        ));

        let prerelease = r#"{"tag_name":"v4.19.0","draft":false,"prerelease":true}"#;
        assert!(matches!(
            parse_latest_release_response(prerelease, "4.18.0", "CloudyTabzy/IMGEditor-Plus"),
            UpdateResult::UpToDate
        ));
    }

    #[test]
    fn latest_release_response_rejects_non_github_html_url() {
        let body = r#"{
            "tag_name": "v4.19.0",
            "html_url": "https://evil.example/payload",
            "draft": false,
            "prerelease": false
        }"#;
        match parse_latest_release_response(body, "4.18.0", "CloudyTabzy/IMGEditor-Plus") {
            UpdateResult::Available { url, .. } => {
                assert_eq!(
                    url,
                    "https://github.com/CloudyTabzy/IMGEditor-Plus/releases/latest"
                );
            }
            other => panic!("expected Available, got {other:?}"),
        }
    }
}
