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

/// Check if ancestor_path is an ancestor directory of descendant_path
/// Returns true if ancestor is a parent directory of descendant
pub fn is_ancestor_path(ancestor: &Path, descendant: &Path) -> bool {
    // Normalize both paths to handle .. and . components
    let ancestor_normalized = normalize_path(ancestor);
    let descendant_normalized = normalize_path(descendant);
    
    // Check if descendant starts with ancestor
    descendant_normalized.starts_with(&ancestor_normalized)
}

/// Extract the path from an access group string
/// Returns None if the access group is not in the expected format
pub fn path_from_access_group(access_group: &str) -> Option<PathBuf> {
    access_group
        .strip_prefix("dir:")
        .map(|path_str| PathBuf::from(path_str))
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
    
    #[test]
    fn test_is_ancestor_path() {
        // Direct parent-child relationship
        assert!(is_ancestor_path(
            Path::new("/home/user/projects"),
            Path::new("/home/user/projects/backend")
        ));
        
        // Grandparent relationship
        assert!(is_ancestor_path(
            Path::new("/home/user"),
            Path::new("/home/user/projects/backend/src")
        ));
        
        // Same path should return true (a directory is its own ancestor)
        assert!(is_ancestor_path(
            Path::new("/home/user/projects"),
            Path::new("/home/user/projects")
        ));
        
        // Not an ancestor
        assert!(!is_ancestor_path(
            Path::new("/home/user/projects/backend"),
            Path::new("/home/user/projects/frontend")
        ));
        
        // Completely different paths
        assert!(!is_ancestor_path(
            Path::new("/home/user"),
            Path::new("/var/log")
        ));
        
        // Child is not ancestor of parent
        assert!(!is_ancestor_path(
            Path::new("/home/user/projects/backend"),
            Path::new("/home/user/projects")
        ));
    }
    
    #[test]
    fn test_path_from_access_group() {
        // Valid access group
        let access_group = "dir:/home/user/projects";
        let path = path_from_access_group(access_group);
        assert_eq!(path, Some(PathBuf::from("/home/user/projects")));
        
        // Invalid access group (no prefix)
        let access_group = "/home/user/projects";
        let path = path_from_access_group(access_group);
        assert_eq!(path, None);
        
        // Invalid access group (wrong prefix)
        let access_group = "file:/home/user/projects";
        let path = path_from_access_group(access_group);
        assert_eq!(path, None);
    }
}