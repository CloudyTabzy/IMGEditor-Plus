//! Session save/restore — persist a named group of archive paths
//! to disk so the user can reopen them as a unit.
//!
//! Adapted from IMGF's `CSession` + `CSessionManager` (Code/Session/).
//! IMGF stores sessions in the Windows registry under
//! `IMGF\Sessions\Data_N`. We persist to a JSON file in the
//! config dir instead — the registry is Windows-only and the
//! format is custom and undocumented.
//!
//! ## Rust-flavored extras over IMGF
//!
//! - **JSON, not a custom binary format** — `serde_json` gives us
//!   schema evolution, round-trip safety, and human-readability
//!   for free. IMGF's "name + ; separated paths" parsing is
//!   fragile (what if a path contains `;`?).
//! - **Pure data type, no UI coupling** — `Session` is `Serialize +
//!   Deserialize` and can be stored anywhere (file, in-memory, future
//!   cloud). The `Sessions` struct is just a `Vec<Session>` wrapper
//!   with file persistence helpers.
//! - **`Default` and `From<Vec<Session>>`** — the loader and saver
//!   both return `Sessions` (or a default on file-not-found), so
//!   callers can chain `let sessions = Sessions::load()?;` without
//!   unwrapping `Option` everywhere.
//! - **No `addEntry(pSession)`-style heap pointers** — IMGF's
//!   `CSessionManager` inherits from `CVectorPool<CSession*>` which
//!   is a `std::vector<CSession*>` of heap-allocated pointers.
//!   The `Session`/`Sessions` split here is value-typed; no manual
//!   `delete` calls needed.
//! - **Path-canonicalization on save** — paths in the file are
//!   canonicalized so the same archive opened via different
//!   mount points doesn't appear as two separate sessions.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const SESSIONS_FILE: &str = "sessions.json";

/// One saved session. A name (for display) plus a list of
/// archive paths to open together. Paths are canonicalized at
/// save time so reloading after a system reboot matches the
/// same archive even if its mount path changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// Display name. Duplicates are allowed (two sessions named
    /// "Mod build" with different archives) because IMGF supports
    /// that and removing the constraint would be a regression.
    pub name: String,
    /// Canonicalized archive paths to reopen.
    pub paths: Vec<PathBuf>,
    /// POSIX timestamp the session was created. Used for
    /// "recently used" sorting in the menu — same convention as
    /// `.db` files (see `parser::db`).
    #[serde(default)]
    pub created_at: u32,
}

impl Session {
    /// Display label for the dropdown — IMGF uses
    /// `"<index>) <name> (<n> tabs)"`. We include the tab count so
    /// the user knows what they're opening.
    pub fn display_label(&self, index: usize) -> String {
        let n = self.paths.len();
        let suffix = if n == 1 { "tab" } else { "tabs" };
        format!("{}. {} ({n} {suffix})", index + 1, self.name)
    }
}

/// The collection of all saved sessions. Persisted to
/// `<config_dir>/sessions.json` on save. Loading returns the
/// default (empty) on file-not-found so callers can chain
/// `let sessions = Sessions::load_or_default();` without
/// `Option`-shuffling.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sessions {
    pub sessions: Vec<Session>,
}

impl Sessions {
    /// Load sessions from the config dir. Returns the default
    /// (empty) on file-not-found; returns `Err` on parse failure
    /// (so the user knows their saved data is corrupted rather
    /// than silently losing it).
    pub fn load() -> Result<Self, SessionError> {
        Self::load_from_path(&config_dir()?.join(SESSIONS_FILE))
    }

    /// Load from a specific path. Used by the tests to avoid
    /// touching the user's real config dir.
    pub fn load_from_path(path: &Path) -> Result<Self, SessionError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(path).map_err(SessionError::Io)?;
        // An empty file is treated as "no sessions" rather than
        // an error — saves a confused user from a corrupt-file
        // toast when the only thing wrong is an empty file.
        if contents.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_str(&contents).map_err(SessionError::Parse)
    }

    /// Persist sessions to the config dir's sessions.json.
    pub fn save(&self) -> Result<(), SessionError> {
        self.save_to_path(&config_dir()?.join(SESSIONS_FILE))
    }

    /// Persist to a specific path.
    pub fn save_to_path(&self, path: &Path) -> Result<(), SessionError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(SessionError::Io)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(SessionError::Serialize)?;
        fs::write(path, json).map_err(SessionError::Io)
    }

    /// Add a session. Path values are canonicalized (best-effort)
    /// so reloading after a system reboot matches the same archive
    /// even if its mount path changed.
    pub fn add(&mut self, name: String, paths: Vec<PathBuf>) {
        let paths: Vec<PathBuf> = paths
            .into_iter()
            .map(|p| p.canonicalize().unwrap_or(p))
            .collect();
        self.sessions.push(Session {
            name,
            paths,
            created_at: now_unix_secs(),
        });
    }

    /// Remove the session at `index`. Panics on out-of-range —
    /// callers validate the index from the UI.
    pub fn remove(&mut self, index: usize) {
        self.sessions.remove(index);
    }

    /// Borrowed iteration over the saved sessions. Returned as
    /// `(&Session, &str label)` for direct use in the dropdown.
    pub fn iter(&self) -> impl Iterator<Item = &Session> {
        self.sessions.iter()
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("I/O error reading or writing session file: {0}")]
    Io(#[from] io::Error),
    #[error("Session file is corrupted and could not be parsed: {0}")]
    Parse(#[source] serde_json::Error),
    #[error("Failed to serialize sessions to JSON: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("Could not determine the config directory for session storage")]
    NoConfigDir,
}

fn config_dir() -> Result<PathBuf, SessionError> {
    let path = if let Ok(app_data) = std::env::var("APPDATA") {
        PathBuf::from(app_data).join("IMGEditor")
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("IMGEditor")
    };
    Ok(path)
}

fn now_unix_secs() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let s = Sessions::default();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let s = Sessions::load_from_path(Path::new("/nonexistent/path/sessions.json"))
            .expect("load missing");
        assert!(s.is_empty());
    }

    #[test]
    fn save_then_load_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sessions.json");
        let mut s = Sessions::default();
        s.add(
            "Mod build".to_string(),
            vec![PathBuf::from("/tmp/a.img"), PathBuf::from("/tmp/b.img")],
        );
        s.add(
            "Test session".to_string(),
            vec![PathBuf::from("/tmp/c.img")],
        );
        s.save_to_path(&path).unwrap();
        let loaded = Sessions::load_from_path(&path).unwrap();
        assert_eq!(s, loaded);
    }

    #[test]
    fn load_corrupt_json_errors_not_panics() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sessions.json");
        std::fs::write(&path, b"this is not json").unwrap();
        let err = Sessions::load_from_path(&path).unwrap_err();
        assert!(matches!(err, SessionError::Parse(_)));
    }

    #[test]
    fn load_empty_file_returns_default() {
        // A file that exists but contains only whitespace isn't an
        // error — it's "no sessions yet". Saves users a corrupt-file
        // toast when the only thing wrong is an empty file.
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sessions.json");
        std::fs::write(&path, b"   \n").unwrap();
        let s = Sessions::load_from_path(&path).unwrap();
        assert!(s.is_empty());
    }

    #[test]
    fn add_canonicalizes_paths() {
        let temp = tempfile::tempdir().unwrap();
        let real = temp.path().join("real.img");
        std::fs::write(&real, b"x").unwrap();
        // Pass a path with redundant components — canonicalize
        // should resolve it.
        let weird = temp.path().join(".").join("real.img");
        let mut s = Sessions::default();
        s.add("t".to_string(), vec![weird]);
        // The stored path is the canonical one, not the input.
        assert_eq!(s.sessions[0].paths[0], real.canonicalize().unwrap());
    }

    #[test]
    fn remove_shrinks_list() {
        let mut s = Sessions::default();
        s.add("a".to_string(), vec![PathBuf::from("/tmp/a")]);
        s.add("b".to_string(), vec![PathBuf::from("/tmp/b")]);
        s.add("c".to_string(), vec![PathBuf::from("/tmp/c")]);
        s.remove(1);
        assert_eq!(s.sessions.len(), 2);
        assert_eq!(s.sessions[0].name, "a");
        assert_eq!(s.sessions[1].name, "c");
    }

    #[test]
    fn display_label_counts_tabs() {
        let one = Session {
            name: "x".to_string(),
            paths: vec![PathBuf::from("/tmp/a")],
            created_at: 0,
        };
        let three = Session {
            name: "x".to_string(),
            paths: vec![
                PathBuf::from("/tmp/a"),
                PathBuf::from("/tmp/b"),
                PathBuf::from("/tmp/c"),
            ],
            created_at: 0,
        };
        assert_eq!(one.display_label(0), "1. x (1 tab)");
        assert_eq!(three.display_label(2), "3. x (3 tabs)");
    }
}
