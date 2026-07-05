//! Property-based tests for Location
//!
//! **Validates: Requirements 3.2**

use super::location::Location;
use proptest::prelude::*;
use std::path::PathBuf;

// Strategy for generating valid path components (non-empty, no path separators)
fn path_component() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,20}".prop_map(|s| s.to_string())
}

// Strategy for generating valid hostnames
fn hostname() -> impl Strategy<Value = String> {
    "[a-z]{3,10}\\.[a-z]{2,5}".prop_map(|s| s.to_string())
}

// Strategy for generating valid provider names
fn provider_name() -> impl Strategy<Value = String> {
    prop::sample::select(vec!["s3", "gcs", "azure", "dropbox"]).prop_map(|s| s.to_string())
}

// Strategy for generating non-root Local locations
fn non_root_local_location() -> impl Strategy<Value = Location> {
    prop::collection::vec(path_component(), 1..5).prop_map(|components| {
        let mut path = PathBuf::from("/");
        for component in components {
            path.push(component);
        }
        Location::Local(path)
    })
}

// Strategy for generating non-root SSH locations
fn non_root_ssh_location() -> impl Strategy<Value = Location> {
    (
        hostname(),
        1u16..65535,
        prop::collection::vec(path_component(), 1..5),
    )
        .prop_map(|(host, port, components)| {
            let mut path = PathBuf::from("/");
            for component in components {
                path.push(component);
            }
            Location::Ssh { host, port, path }
        })
}

// Strategy for generating non-root Cloud locations
fn non_root_cloud_location() -> impl Strategy<Value = Location> {
    (
        provider_name(),
        path_component(),
        prop::collection::vec(path_component(), 1..5),
    )
        .prop_map(|(provider, bucket, components)| {
            let mut path = PathBuf::new();
            for component in components {
                path.push(component);
            }
            Location::Cloud {
                provider,
                bucket,
                path,
            }
        })
}

// Strategy for generating non-root Archive locations
fn non_root_archive_location() -> impl Strategy<Value = Location> {
    (
        non_root_local_location(),
        prop::collection::vec(path_component(), 1..3),
    )
        .prop_map(|(archive_path, components)| {
            let mut inner_path = PathBuf::new();
            for component in components {
                inner_path.push(component);
            }
            Location::Archive {
                archive_path: Box::new(archive_path),
                inner_path,
            }
        })
}

// Strategy for generating any non-root Location
fn non_root_location() -> impl Strategy<Value = Location> {
    prop_oneof![
        non_root_local_location(),
        non_root_ssh_location(),
        non_root_cloud_location(),
        non_root_archive_location(),
    ]
}

proptest! {
    /// **Property 7: Parent Navigation**
    ///
    /// For any non-root Location, the parent() method should return Some(parent_location),
    /// and navigating to the parent then back to a child should preserve the path structure.
    ///
    /// **Validates: Requirements 3.2**
    #[test]
    fn prop_parent_navigation_returns_some_for_non_root(location in non_root_location()) {
        // For any non-root location, parent() should return Some
        let parent = location.parent();
        prop_assert!(parent.is_some(), "Non-root location {:?} should have a parent", location);
    }

    /// **Property 7: Parent Navigation (Round-trip)**
    ///
    /// For any non-root Location, navigating to parent then joining with the last component
    /// should preserve the path structure.
    ///
    /// **Validates: Requirements 3.2**
    #[test]
    fn prop_parent_child_roundtrip(location in non_root_location()) {
        // Get the parent
        let parent = location.parent();
        prop_assert!(parent.is_some(), "Non-root location should have a parent");

        let parent = parent.unwrap();

        // Extract the last component from the original location
        let last_component = match &location {
            Location::Local(path) => {
                path.file_name().and_then(|s| s.to_str()).map(|s| s.to_string())
            }
            Location::Ssh { path, .. } => {
                path.file_name().and_then(|s| s.to_str()).map(|s| s.to_string())
            }
            Location::Cloud { path, .. } => {
                path.file_name().and_then(|s| s.to_str()).map(|s| s.to_string())
            }
            Location::Archive { inner_path, .. } => {
                inner_path.file_name().and_then(|s| s.to_str()).map(|s| s.to_string())
            }
        };

        // If we have a last component, joining parent with it should give us back the original location
        if let Some(component) = last_component {
            let reconstructed = parent.join(&component);
            prop_assert_eq!(
                reconstructed,
                location,
                "Parent-child round-trip failed: parent.join(component) != original"
            );
        }
    }

    /// **Property 7: Parent Navigation (Repeated parent calls reach root)**
    ///
    /// For any Location, repeatedly calling parent() should eventually return None (root).
    ///
    /// **Validates: Requirements 3.2**
    #[test]
    fn prop_repeated_parent_reaches_root(location in non_root_location()) {
        let mut current = Some(location);
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 100; // Safety limit to prevent infinite loops

        // Keep calling parent() until we reach None (root)
        while let Some(loc) = current {
            current = loc.parent();
            iterations += 1;

            prop_assert!(
                iterations < MAX_ITERATIONS,
                "Failed to reach root after {} iterations, possible infinite loop",
                MAX_ITERATIONS
            );
        }

        // We should have reached None (root) within reasonable iterations
        prop_assert!(iterations > 0, "Should have taken at least one step to reach root");
    }

    /// **Property 7: Parent Navigation (Parent is shorter)**
    ///
    /// For any non-root Location, the parent's display path should be a prefix of the child's path
    /// (or in the case of Archive exit, should be the archive location itself).
    ///
    /// **Validates: Requirements 3.2**
    #[test]
    fn prop_parent_is_prefix_or_archive_exit(location in non_root_location()) {
        let parent = location.parent();
        prop_assert!(parent.is_some(), "Non-root location should have a parent");

        let parent = parent.unwrap();
        let child_path = location.display_path();
        let parent_path = parent.display_path();

        // For Archive locations exiting to filesystem, the parent might not be a prefix
        // (e.g., "/path/to/archive.zip#" -> "/path/to/archive.zip")
        // For all other cases, parent path should be a prefix of child path
        match &location {
            Location::Archive { archive_path, inner_path } => {
                // If inner_path is at root of archive, parent is the archive location
                if inner_path.parent().is_none() || inner_path.as_os_str().is_empty() {
                    prop_assert_eq!(
                        parent,
                        (**archive_path).clone(),
                        "Archive root parent should be the archive location"
                    );
                } else {
                    // Otherwise, parent should be within the archive
                    prop_assert!(
                        child_path.starts_with(&parent_path) || parent_path.len() < child_path.len(),
                        "Parent path should be shorter or a prefix: parent='{}', child='{}'",
                        parent_path,
                        child_path
                    );
                }
            }
            _ => {
                // For non-archive locations, parent path should be a prefix
                prop_assert!(
                    child_path.starts_with(&parent_path) || parent_path.len() < child_path.len(),
                    "Parent path should be shorter or a prefix: parent='{}', child='{}'",
                    parent_path,
                    child_path
                );
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_local_root_has_no_parent() {
        let root = Location::Local(PathBuf::from("/"));
        assert!(
            root.parent().is_none(),
            "Root location should have no parent"
        );
    }

    #[test]
    fn test_ssh_root_has_no_parent() {
        let root = Location::Ssh {
            host: "example.com".to_string(),
            port: 22,
            path: PathBuf::from("/"),
        };
        assert!(
            root.parent().is_none(),
            "SSH root location should have no parent"
        );
    }

    #[test]
    fn test_cloud_root_has_no_parent() {
        let root = Location::Cloud {
            provider: "s3".to_string(),
            bucket: "my-bucket".to_string(),
            path: PathBuf::from(""),
        };
        assert!(
            root.parent().is_none(),
            "Cloud root location should have no parent"
        );
    }

    #[test]
    fn test_archive_root_exits_to_filesystem() {
        let archive_path = Location::Local(PathBuf::from("/home/user/archive.zip"));
        let archive_root = Location::Archive {
            archive_path: Box::new(archive_path.clone()),
            inner_path: PathBuf::from(""),
        };

        let parent = archive_root.parent();
        assert_eq!(
            parent,
            Some(Location::Local(PathBuf::from("/home/user"))),
            "Archive root parent should be the parent directory of the archive"
        );
    }
}
