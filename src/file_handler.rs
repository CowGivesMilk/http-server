//file_handler
use anyhow::{Context, Result};
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
pub struct FileHandler {
    target_dir: PathBuf, // canonicalized, absolute
}

impl FileHandler {
    pub fn try_new(target_dir: impl Into<PathBuf>) -> Result<Self> {
        let td = target_dir.into();
        let target_dir = td
            .canonicalize()
            .with_context(|| format!("target dir {} is not accessible", td.display()))?;
        if !target_dir.is_dir() {
            anyhow::bail!("target dir {} is not a directory", target_dir.display());
        }
        Ok(FileHandler { target_dir })
    }

    pub fn get_file_data(&self, uri_path: &str) -> Result<Option<Vec<u8>>> {
        let rel = match sanitize(uri_path) {
            Some(rel) => rel,
            None => return Ok(None), // traversal / bad path → treat as not found
        };
        let file_path = self.target_dir.join(&rel);

        let canonical = match file_path.canonicalize() {
            Ok(p) => p,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(e).with_context(|| format!("canonicalize {}", file_path.display()));
            }
        };
        if !canonical.starts_with(&self.target_dir) {
            return Ok(None);
        }

        match std::fs::read(&canonical) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("read {}", canonical.display())),
        }
    }

    pub fn write_to_file(&self, uri_path: &str, content: &str) -> bool {
        let Some(rel) = sanitize(uri_path) else {
            return false;
        };

        let full = self.target_dir.join(&rel);

        // Defense in depth: ensure we didn't escape `base` via `..` etc.
        match full.canonicalize() {
            Ok(real) => {
                if !real.starts_with(&self.target_dir) {
                    return false;
                }
            }
            Err(_) => {
                // Parent may not exist yet — check lexically
                if full
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                {
                    return false;
                }
            }
        }

        if let Some(parent) = full.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return false;
            }
        }

        std::fs::write(&full, content).is_ok()
    }
}

/// Turn a request URI path into a safe relative path, or None if it escapes.
fn sanitize(uri_path: &str) -> Option<PathBuf> {
    // Strip query string if present.
    let path = uri_path.split(['?', '#']).next().unwrap_or(uri_path);
    // Decode percent-encoding here if you support it (see note below).
    let mut out = PathBuf::new();
    for comp in Path::new(path).components() {
        match comp {
            Component::Normal(seg) => out.push(seg),
            Component::RootDir | Component::CurDir => {} // drop leading '/', '.'
            Component::ParentDir => return None,         // reject '..'
            Component::Prefix(_) => return None,         // Windows drive letters
        }
    }
    Some(out)
}
