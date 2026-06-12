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

pub struct Updater {
    repo: String,
    current_version: String,
    state: UpdateState,
    sender: async_channel::Sender<UpdateResult>,
    receiver: async_channel::Receiver<UpdateResult>,
}

impl Updater {
    pub fn new(repo: impl Into<String>, current_version: impl Into<String>) -> Self {
        let (sender, receiver) = async_channel::bounded(1);
        Self {
            repo: repo.into(),
            current_version: current_version.into(),
            state: UpdateState::Idle,
            sender,
            receiver,
        }
    }

    pub fn state(&self) -> &UpdateState {
        &self.state
    }

    pub fn check(&mut self) {
        if matches!(self.state, UpdateState::Checking) {
            return;
        }

        self.state = UpdateState::Checking;
        let url = format!("https://api.github.com/repos/{}/tags", self.repo);
        let repo = self.repo.clone();
        let current = self.current_version.clone();
        let sender = self.sender.clone();

        smol::spawn(async move {
            let result = smol::unblock(move || fetch_tags(&url, &repo, &current)).await;
            let _ = sender.try_send(result);
        })
        .detach();
    }

    pub fn poll(&mut self) -> Option<UpdateResult> {
        if let Ok(result) = self.receiver.try_recv() {
            self.state = match &result {
                UpdateResult::Available { version, url } => UpdateState::Available {
                    version: version.clone(),
                    url: url.clone(),
                },
                UpdateResult::UpToDate => UpdateState::UpToDate,
                UpdateResult::Error(message) => UpdateState::Error(message.clone()),
            };
            return Some(result);
        }
        None
    }
}

fn fetch_tags(url: &str, repo: &str, current_version: &str) -> UpdateResult {
    let response = match ureq::get(url).set("User-Agent", "IMGEditor").call() {
        Ok(response) => response,
        Err(error) => return UpdateResult::Error(error.to_string()),
    };

    let body = match response.into_string() {
        Ok(body) => body,
        Err(error) => return UpdateResult::Error(error.to_string()),
    };

    let value: serde_json::Value = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(error) => return UpdateResult::Error(error.to_string()),
    };

    let array = match value.as_array() {
        Some(array) => array,
        None => return UpdateResult::Error("unexpected GitHub response".to_string()),
    };

    let first = match array.first() {
        Some(first) => first,
        None => return UpdateResult::UpToDate,
    };

    let tag_name = match first.get("name").and_then(|name| name.as_str()) {
        Some(name) => name,
        None => return UpdateResult::Error("missing tag name".to_string()),
    };

    let latest = match parse_version(tag_name) {
        Some(version) => version,
        None => return UpdateResult::Error(format!("invalid tag version: {tag_name}")),
    };

    let current = match parse_version(current_version) {
        Some(version) => version,
        None => return UpdateResult::Error(format!("invalid current version: {current_version}")),
    };

    if latest > current {
        UpdateResult::Available {
            version: tag_name.to_string(),
            url: format!("https://github.com/{}/releases", repo),
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
}
