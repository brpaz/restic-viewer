//! Persists the list of Repositories the app knows about, minus any secret.
//!
//! Non-secret metadata (display name, Backend type, path/endpoint) lives here in a TOML
//! file. The Repository Password and any Backend credential are never written here —
//! see `keyring` for those.

use std::fs;
use std::path::PathBuf;

use restic_client::Backend;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepositoryEntry {
    pub id: String,
    pub name: String,
    pub backend: Backend,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    repositories: Vec<RepositoryEntry>,
}

pub struct RepositoryStore {
    path: PathBuf,
}

impl RepositoryStore {
    pub fn new() -> Self {
        Self {
            path: glib::user_config_dir()
                .join("restic-viewer")
                .join("repositories.toml"),
        }
    }

    #[cfg(test)]
    fn at_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Vec<RepositoryEntry> {
        let Ok(contents) = fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        toml::from_str::<ConfigFile>(&contents)
            .map(|c| c.repositories)
            .unwrap_or_default()
    }

    pub fn add(&self, entry: RepositoryEntry) -> std::io::Result<()> {
        let mut entries = self.load();
        entries.push(entry);
        self.save(&entries)
    }

    /// Only renames the app's local display name — never touches the Repository itself.
    pub fn rename(&self, id: &str, new_name: &str) -> std::io::Result<()> {
        let mut entries = self.load();
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
            entry.name = new_name.to_string();
        }
        self.save(&entries)
    }

    /// Only forgets the Repository locally (this config entry) — never touches the
    /// Repository itself or its backing storage. Callers are responsible for also
    /// deleting its keyring credentials.
    pub fn remove(&self, id: &str) -> std::io::Result<()> {
        let mut entries = self.load();
        entries.retain(|e| e.id != id);
        self.save(&entries)
    }

    fn save(&self, entries: &[RepositoryEntry]) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(&ConfigFile {
            repositories: entries.to_vec(),
        })
        .expect("RepositoryEntry always serializes to valid TOML");
        fs::write(&self.path, contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str) -> RepositoryEntry {
        RepositoryEntry {
            id: id.to_string(),
            name: format!("Repo {id}"),
            backend: Backend::Local {
                path: format!("/tmp/{id}").into(),
            },
        }
    }

    #[test]
    fn round_trips_through_the_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = RepositoryStore::at_path(dir.path().join("repositories.toml"));

        assert_eq!(store.load(), Vec::new());

        store.add(entry("a")).unwrap();
        store.add(entry("b")).unwrap();

        let loaded = store.load();
        assert_eq!(loaded, vec![entry("a"), entry("b")]);
    }

    #[test]
    fn rename_updates_only_the_matching_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = RepositoryStore::at_path(dir.path().join("repositories.toml"));
        store.add(entry("a")).unwrap();
        store.add(entry("b")).unwrap();

        store.rename("a", "New Name").unwrap();

        let loaded = store.load();
        assert_eq!(loaded[0].name, "New Name");
        assert_eq!(loaded[1].name, "Repo b");
    }

    #[test]
    fn remove_deletes_only_the_matching_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = RepositoryStore::at_path(dir.path().join("repositories.toml"));
        store.add(entry("a")).unwrap();
        store.add(entry("b")).unwrap();

        store.remove("a").unwrap();

        assert_eq!(store.load(), vec![entry("b")]);
    }

    #[test]
    fn never_writes_a_password_field() {
        let dir = tempfile::tempdir().unwrap();
        let store = RepositoryStore::at_path(dir.path().join("repositories.toml"));
        store.add(entry("a")).unwrap();

        let raw = fs::read_to_string(dir.path().join("repositories.toml")).unwrap();
        assert!(!raw.to_lowercase().contains("password"));
    }
}
