use fs2::FileExt;
use qit_domain::{RegistryError, RegistryStore, WorkspaceId, WorkspaceRecord};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ENV_DATA_DIR: &str = "QIT_DATA_DIR";

#[derive(Debug, Clone)]
pub struct FilesystemRegistry {
    data_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RegistryFile {
    workspaces: HashMap<String, WorkspaceRecord>,
}

impl FilesystemRegistry {
    pub fn new() -> Result<Self, String> {
        let data_root = if let Some(root) = std::env::var_os(ENV_DATA_DIR) {
            PathBuf::from(root)
        } else {
            directories::ProjectDirs::from("com", "quickgit", "Qit")
                .map(|dirs| dirs.data_local_dir().to_path_buf())
                .ok_or_else(|| "no home directory available for qit data".to_string())?
        };

        Ok(Self { data_root })
    }

    pub fn with_root(data_root: PathBuf) -> Self {
        Self { data_root }
    }

    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    pub fn registry_path(&self) -> PathBuf {
        self.data_root.join("registry.json")
    }

    fn lock_path(&self) -> PathBuf {
        self.data_root.join("registry.lock")
    }

    pub fn repos_dir(&self) -> PathBuf {
        self.data_root.join("repos")
    }

    fn ensure_data_root(&self) -> Result<(), RegistryError> {
        std::fs::create_dir_all(&self.data_root).map_err(|err| RegistryError::Io {
            operation: "create data directory",
            path: self.data_root.clone(),
            message: err.to_string(),
        })
    }

    fn load_registry(&self) -> Result<RegistryFile, RegistryError> {
        let path = self.registry_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                serde_json::from_str(&contents).map_err(|err| RegistryError::CorruptRegistry {
                    path,
                    message: err.to_string(),
                })
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(RegistryFile::default()),
            Err(err) => Err(RegistryError::Io {
                operation: "read registry",
                path,
                message: err.to_string(),
            }),
        }
    }

    fn lock_registry(&self) -> Result<File, RegistryError> {
        self.ensure_data_root()?;
        let path = self.lock_path();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|err| RegistryError::Io {
                operation: "open registry lock",
                path: path.clone(),
                message: err.to_string(),
            })?;
        file.lock_exclusive().map_err(|err| RegistryError::Io {
            operation: "lock registry",
            path,
            message: err.to_string(),
        })?;
        Ok(file)
    }

    fn temp_registry_path(&self) -> PathBuf {
        let pid = std::process::id();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        self.data_root
            .join(format!("registry.json.tmp-{pid}-{nonce}"))
    }

    fn write_registry_atomic(&self, registry: &RegistryFile) -> Result<(), RegistryError> {
        let encoded = serde_json::to_string_pretty(registry).map_err(|err| RegistryError::Io {
            operation: "encode registry",
            path: self.registry_path(),
            message: err.to_string(),
        })?;
        let temp_path = self.temp_registry_path();
        let mut temp = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|err| RegistryError::Io {
                operation: "create temp registry",
                path: temp_path.clone(),
                message: err.to_string(),
            })?;
        use std::io::Write as _;
        temp.write_all(encoded.as_bytes())
            .and_then(|_| temp.sync_all())
            .map_err(|err| RegistryError::Io {
                operation: "write temp registry",
                path: temp_path.clone(),
                message: err.to_string(),
            })?;
        std::fs::rename(&temp_path, self.registry_path()).map_err(|err| RegistryError::Io {
            operation: "replace registry",
            path: self.registry_path(),
            message: err.to_string(),
        })?;
        self.sync_data_root()
    }

    #[cfg(unix)]
    fn sync_data_root(&self) -> Result<(), RegistryError> {
        File::open(&self.data_root)
            .and_then(|dir| dir.sync_all())
            .map_err(|err| RegistryError::Io {
                operation: "sync registry directory",
                path: self.data_root.clone(),
                message: err.to_string(),
            })
    }

    #[cfg(not(unix))]
    fn sync_data_root(&self) -> Result<(), RegistryError> {
        Ok(())
    }
}

impl RegistryStore for FilesystemRegistry {
    fn canonical_worktree(&self, worktree: &Path) -> Result<PathBuf, RegistryError> {
        let absolute = if worktree.is_absolute() {
            worktree.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|err| RegistryError::CurrentDirectory(err.to_string()))?
                .join(worktree)
        };
        if !absolute.exists() {
            return Err(RegistryError::MissingWorktree(absolute));
        }
        if !absolute.is_dir() {
            return Err(RegistryError::WorktreeNotDirectory(absolute));
        }
        dunce::canonicalize(&absolute).map_err(|err| RegistryError::Io {
            operation: "canonicalize worktree",
            path: absolute,
            message: err.to_string(),
        })
    }

    fn default_sidecar_path(&self, id: WorkspaceId) -> Result<PathBuf, RegistryError> {
        std::fs::create_dir_all(self.repos_dir()).map_err(|err| RegistryError::Io {
            operation: "create repos directory",
            path: self.repos_dir(),
            message: err.to_string(),
        })?;
        Ok(self.repos_dir().join(format!("{}.git", id.0)))
    }

    fn load(&self, id: WorkspaceId) -> Result<Option<WorkspaceRecord>, RegistryError> {
        Ok(self
            .load_registry()?
            .workspaces
            .get(&id.0.to_string())
            .cloned())
    }

    fn save(&self, id: WorkspaceId, record: WorkspaceRecord) -> Result<(), RegistryError> {
        let _lock = self.lock_registry()?;
        let mut registry = self.load_registry()?;
        registry.workspaces.insert(id.0.to_string(), record);
        self.write_registry_atomic(&registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use uuid::Uuid;

    #[test]
    fn registry_round_trip_and_worktree_lookup() {
        let temp = TempDir::new().unwrap();
        let registry = FilesystemRegistry::with_root(temp.path().join("qit-data"));
        let worktree = temp.path().join("worktree");
        let id = WorkspaceId(Uuid::new_v4());
        let record = WorkspaceRecord {
            worktree: worktree.clone(),
            sidecar: temp.path().join("repos/sidecar.git"),
            exported_branch: "main".into(),
            checked_out_branch: Some("main".into()),
        };

        registry.save(id, record.clone()).unwrap();

        let loaded = registry.load(id).unwrap().unwrap();
        assert_eq!(loaded, record);
    }

    #[test]
    fn canonical_worktree_returns_absolute_path() {
        let temp = TempDir::new().unwrap();
        let registry = FilesystemRegistry::with_root(temp.path().join("qit-data"));
        let worktree = temp.path().join("nested");
        std::fs::create_dir_all(&worktree).unwrap();

        let canonical = registry.canonical_worktree(&worktree).unwrap();
        assert!(canonical.is_absolute());
    }

    #[test]
    fn canonical_worktree_rejects_missing_and_non_directory_paths() {
        let temp = TempDir::new().unwrap();
        let registry = FilesystemRegistry::with_root(temp.path().join("qit-data"));
        let missing = temp.path().join("missing");
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "not a dir").unwrap();

        assert!(matches!(
            registry.canonical_worktree(&missing),
            Err(RegistryError::MissingWorktree(path)) if path == missing
        ));
        assert!(matches!(
            registry.canonical_worktree(&file),
            Err(RegistryError::WorktreeNotDirectory(path)) if path == file
        ));
    }

    #[test]
    fn load_registry_fails_loudly_on_invalid_json() {
        let temp = TempDir::new().unwrap();
        let registry = FilesystemRegistry::with_root(temp.path().join("qit-data"));
        std::fs::create_dir_all(registry.data_root()).unwrap();
        std::fs::write(registry.registry_path(), "{not valid json").unwrap();

        assert!(matches!(
            registry.load_registry(),
            Err(RegistryError::CorruptRegistry { .. })
        ));
    }
}
