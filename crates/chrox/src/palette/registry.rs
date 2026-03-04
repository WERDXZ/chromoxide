use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{user::PaletteFile, Palette};

#[derive(Debug, Clone, Default)]
pub struct PaletteRegistry {
    user: HashMap<String, UserPaletteRecord>,
}

#[derive(Debug, Clone)]
pub struct UserPaletteRecord {
    pub id: String,
    pub path: PathBuf,
    pub palette: PaletteFile,
}

impl PaletteRegistry {
    pub fn discover(search_paths: &[PathBuf]) -> Result<Self, Error> {
        let mut palette_files = Vec::new();
        for root in search_paths {
            collect_palette_files(root, &mut palette_files)?;
        }

        palette_files.sort();
        palette_files.dedup();

        let mut user: HashMap<String, UserPaletteRecord> = HashMap::new();
        for path in palette_files {
            let palette = PaletteFile::from_path(&path).map_err(|source| Error::LoadPalette {
                path: path.clone(),
                source: Box::new(source),
            })?;
            let id = palette.id();

            if let Some(existing) = user.get(&id) {
                return Err(Error::DuplicateId {
                    id,
                    first: existing.path.clone(),
                    second: path,
                });
            }

            user.insert(id.clone(), UserPaletteRecord { id, path, palette });
        }

        Ok(Self { user })
    }

    pub fn user_palette(&self, id: &str) -> Option<&PaletteFile> {
        self.user.get(id).map(|entry| &entry.palette)
    }

    pub fn user_record(&self, id: &str) -> Option<&UserPaletteRecord> {
        self.user.get(id)
    }

    pub fn user_palettes(&self) -> impl Iterator<Item = &UserPaletteRecord> {
        self.user.values()
    }

    pub fn user_palette_count(&self) -> usize {
        self.user.len()
    }
}

fn collect_palette_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), Error> {
    if root.is_file() {
        if is_toml(root) {
            out.push(root.to_path_buf());
        }
        return Ok(());
    }

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|source| Error::ReadDir {
            path: dir.clone(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| Error::ReadDirEntry {
                path: dir.clone(),
                source,
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|source| Error::FileType {
                path: path.clone(),
                source,
            })?;

            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() && is_toml(&path) {
                out.push(path);
            }
        }
    }

    Ok(())
}

fn is_toml(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read palette directory `{path}`")]
    ReadDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read palette directory entry in `{path}`")]
    ReadDirEntry {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to inspect palette path `{path}`")]
    FileType {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to load palette file `{path}`")]
    LoadPalette {
        path: PathBuf,
        #[source]
        source: Box<super::user::Error>,
    },
    #[error("duplicate palette id `{id}` from `{first}` and `{second}`")]
    DuplicateId {
        id: String,
        first: PathBuf,
        second: PathBuf,
    },
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{Error, PaletteRegistry};

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "chrox-palette-registry-test-{nanos}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn discovers_palettes_from_directory_tree() {
        let root = unique_temp_dir();
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).expect("test directories should be created");

        let first = root.join("one.toml");
        std::fs::write(
            &first,
            r#"
name = "One"
"#,
        )
        .expect("first palette should be written");

        let second = nested.join("two.toml");
        std::fs::write(
            &second,
            r#"
name = "Two"
"#,
        )
        .expect("second palette should be written");

        let ignored = nested.join("ignore.txt");
        std::fs::write(&ignored, "name = \"Ignored\"").expect("ignored file should be written");

        let registry =
            PaletteRegistry::discover(std::slice::from_ref(&root)).expect("discovery should work");

        assert_eq!(registry.user_palette_count(), 2);
        assert!(registry.user_palette("one").is_some());
        assert!(registry.user_palette("two").is_some());

        let _ = std::fs::remove_file(first);
        let _ = std::fs::remove_file(second);
        let _ = std::fs::remove_file(ignored);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(&root).expect("test directory should be created");

        let first = root.join("a.toml");
        std::fs::write(
            &first,
            r#"
name = "Shared"
"#,
        )
        .expect("first palette should be written");

        let second = root.join("b.toml");
        std::fs::write(
            &second,
            r#"
name = "shared"
"#,
        )
        .expect("second palette should be written");

        let err = PaletteRegistry::discover(std::slice::from_ref(&root))
            .expect_err("duplicate ids should fail");
        assert!(matches!(err, Error::DuplicateId { id, .. } if id == "shared"));

        let _ = std::fs::remove_file(first);
        let _ = std::fs::remove_file(second);
        let _ = std::fs::remove_dir_all(root);
    }
}
