use std::path::{Path, PathBuf};
use std::io::{self, Write};
use serde::{Deserialize, Serialize};
use directories::ProjectDirs;
use crate::tree::SnowflakeTree;

// ── File format ────────────────────────────────────────────────────────────────
// The .snowflake format is a plain-text JSON file with a human-readable header:
//
//   # Snowflake Story File — readable even without the app
//   # Format: JSON, version 1
//   # Each card has: id, label, text, children (null or [id, id, id])
//   { "version": 1, "tree": { ... } }
//
// ──────────────────────────────────────────────────────────────────────────────

const FILE_HEADER: &str = "\
# Snowflake Story File — readable even without the app
# Format: JSON (version 1)
# Fields:
#   version   — format version number
#   tree      — the story tree
#   tree.roots — [beginning_id, middle_id, end_id]
#   tree.cards — map of id → { id, label, text, children }
#     children  — null (leaf) or [id, id, id] (expanded into three sub-cards)
#
# You can read the story by following roots → children recursively.
";

#[derive(Debug, Serialize, Deserialize)]
struct SnowflakeFile {
    version: u32,
    tree: SnowflakeTree,
}

pub fn save(path: &Path, tree: &SnowflakeTree) -> io::Result<()> {
    let file_data = SnowflakeFile {
        version: 1,
        tree: tree.clone(),
    };
    let json = serde_json::to_string_pretty(&file_data)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let mut f = std::fs::File::create(path)?;
    f.write_all(FILE_HEADER.as_bytes())?;
    f.write_all(json.as_bytes())?;
    Ok(())
}

pub fn load(path: &Path) -> io::Result<SnowflakeTree> {
    let raw = std::fs::read_to_string(path)?;
    // Strip comment lines before parsing JSON
    let json: String = raw
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let file_data: SnowflakeFile = serde_json::from_str(&json)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(file_data.tree)
}

// ── Session persistence ────────────────────────────────────────────────────────

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Session {
    /// Path of the last opened file (if any).
    pub last_file: Option<PathBuf>,
}

fn session_path() -> Option<PathBuf> {
    ProjectDirs::from("io", "snowflake", "snowflake")
        .map(|d| d.data_local_dir().join("session.json"))
}

pub fn load_session() -> Session {
    let Some(path) = session_path() else {
        return Session::default();
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Session::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_session(session: &Session) {
    let Some(path) = session_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(session) {
        let _ = std::fs::write(&path, json);
    }
}
