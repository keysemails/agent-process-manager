//! Utility functions for Agent Process Manager

use std::path::{Path, PathBuf};

/// Normalize a directory path to a canonical form
/// This ensures consistent access group naming regardless of how the path is specified
pub fn normalize_path(path: &Path) -> PathBuf {
    // Try to canonicalize the path (resolves symlinks, .. and .)
    // If it fails (e.g., path doesn't exist), fall back to normalizing components
    match path.canonicalize() {
        Ok(canonical) => canonical,
        Err(_) => {
            // If canonicalize fails, at least clean up the path
            let mut normalized = PathBuf::new();
            for component in path.components() {
                match component {
                    std::path::Component::ParentDir => {
                        normalized.pop();
                    }
                    std::path::Component::CurDir => {
                        // Skip "." components
                    }
                    _ => {
                        normalized.push(component);
                    }
                }
            }
            normalized
        }
    }
}

/// Create an access group identifier from a directory path
pub fn access_group_from_dir(dir: &Path) -> String {
    let normalized = normalize_path(dir);
    format!("dir:{}", normalized.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        // Test basic normalization
        let path = Path::new("/home/user/./projects/../projects/app");
        let normalized = normalize_path(path);
        assert!(normalized.to_string_lossy().contains("projects/app"));
        
        // Test already normalized path
        let path = Path::new("/home/user/projects");
        let normalized = normalize_path(path);
        // Since the path might not exist, we just check it's not empty
        assert!(!normalized.to_string_lossy().is_empty());
    }

    #[test]
    fn test_access_group_from_dir() {
        let dir = Path::new("/home/user/projects");
        let access_group = access_group_from_dir(dir);
        assert!(access_group.starts_with("dir:"));
        assert!(access_group.contains("projects"));
    }
}