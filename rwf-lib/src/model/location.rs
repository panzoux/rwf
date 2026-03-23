//! Location abstraction for different storage types

use std::path::PathBuf;

/// Abstract representation of file locations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Location {
    Local(PathBuf),
    Ssh {
        host: String,
        port: u16,
        path: PathBuf,
    },
    Cloud {
        provider: String,
        bucket: String,
        path: PathBuf,
    },
    Archive {
        archive_path: Box<Location>,
        inner_path: PathBuf,
    },
}

impl Location {
    /// Get display path for UI
    pub fn display_path(&self) -> String {
        match self {
            Location::Local(path) => path.display().to_string(),
            Location::Ssh { host, port, path } => {
                format!("ssh://{}:{}{}", host, port, path.display())
            }
            Location::Cloud { provider, bucket, path } => {
                format!("{}://{}/{}", provider, bucket, path.display())
            }
            Location::Archive { archive_path, inner_path } => {
                format!("{}#{}", archive_path.display_path(), inner_path.display())
            }
        }
    }

    /// Get underlying PathBuf for Local locations
    pub fn path(&self) -> Option<&std::path::Path> {
        match self {
            Location::Local(path) => Some(path),
            _ => None,
        }
    }

    /// Get parent location
    pub fn parent(&self) -> Option<Location> {
        match self {
            Location::Local(path) => {
                path.parent().map(|p| Location::Local(p.to_path_buf()))
            }
            Location::Ssh { host, port, path } => {
                path.parent().map(|p| Location::Ssh {
                    host: host.clone(),
                    port: *port,
                    path: p.to_path_buf(),
                })
            }
            Location::Cloud { provider, bucket, path } => {
                path.parent().map(|p| Location::Cloud {
                    provider: provider.clone(),
                    bucket: bucket.clone(),
                    path: p.to_path_buf(),
                })
            }
            Location::Archive { archive_path, inner_path } => {
                if inner_path.parent().is_some() && inner_path != &PathBuf::new() {
                    // Navigate up within the archive
                    inner_path.parent().map(|p| Location::Archive {
                        archive_path: archive_path.clone(),
                        inner_path: p.to_path_buf(),
                    })
                } else {
                    // Exit archive, return to the directory containing the archive
                    archive_path.parent()
                }
            }
        }
    }
    
    /// Join with a path component
    pub fn join(&self, component: &str) -> Location {
        match self {
            Location::Local(path) => Location::Local(path.join(component)),
            Location::Ssh { host, port, path } => Location::Ssh {
                host: host.clone(),
                port: *port,
                path: path.join(component),
            },
            Location::Cloud { provider, bucket, path } => Location::Cloud {
                provider: provider.clone(),
                bucket: bucket.clone(),
                path: path.join(component),
            },
            Location::Archive { archive_path, inner_path } => Location::Archive {
                archive_path: archive_path.clone(),
                inner_path: inner_path.join(component),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_display_path() {
        let loc = Location::Local(PathBuf::from("/home/user/documents"));
        assert_eq!(loc.display_path(), "/home/user/documents");
    }

    #[test]
    fn test_ssh_display_path() {
        let loc = Location::Ssh {
            host: "example.com".to_string(),
            port: 22,
            path: PathBuf::from("/var/www"),
        };
        assert_eq!(loc.display_path(), "ssh://example.com:22/var/www");
    }

    #[test]
    fn test_cloud_display_path() {
        let loc = Location::Cloud {
            provider: "s3".to_string(),
            bucket: "my-bucket".to_string(),
            path: PathBuf::from("folder/file.txt"),
        };
        assert_eq!(loc.display_path(), "s3://my-bucket/folder/file.txt");
    }

    #[test]
    fn test_archive_display_path() {
        let archive_loc = Box::new(Location::Local(PathBuf::from("/home/user/archive.zip")));
        let loc = Location::Archive {
            archive_path: archive_loc,
            inner_path: PathBuf::from("folder/file.txt"),
        };
        assert_eq!(loc.display_path(), "/home/user/archive.zip#folder/file.txt");
    }

    #[test]
    fn test_local_parent() {
        let loc = Location::Local(PathBuf::from("/home/user/documents"));
        let parent = loc.parent().unwrap();
        assert_eq!(parent, Location::Local(PathBuf::from("/home/user")));
    }

    #[test]
    fn test_local_root_parent() {
        let loc = Location::Local(PathBuf::from("/"));
        assert!(loc.parent().is_none());
    }

    #[test]
    fn test_ssh_parent() {
        let loc = Location::Ssh {
            host: "example.com".to_string(),
            port: 22,
            path: PathBuf::from("/var/www/html"),
        };
        let parent = loc.parent().unwrap();
        assert_eq!(
            parent,
            Location::Ssh {
                host: "example.com".to_string(),
                port: 22,
                path: PathBuf::from("/var/www"),
            }
        );
    }

    #[test]
    fn test_cloud_parent() {
        let loc = Location::Cloud {
            provider: "s3".to_string(),
            bucket: "my-bucket".to_string(),
            path: PathBuf::from("folder/subfolder"),
        };
        let parent = loc.parent().unwrap();
        assert_eq!(
            parent,
            Location::Cloud {
                provider: "s3".to_string(),
                bucket: "my-bucket".to_string(),
                path: PathBuf::from("folder"),
            }
        );
    }

    #[test]
    fn test_archive_parent_within_archive() {
        let archive_loc = Box::new(Location::Local(PathBuf::from("/home/user/archive.zip")));
        let loc = Location::Archive {
            archive_path: archive_loc.clone(),
            inner_path: PathBuf::from("folder/subfolder"),
        };
        let parent = loc.parent().unwrap();
        assert_eq!(
            parent,
            Location::Archive {
                archive_path: archive_loc,
                inner_path: PathBuf::from("folder"),
            }
        );
    }

    #[test]
    fn test_archive_parent_exit_archive() {
        let archive_loc = Box::new(Location::Local(PathBuf::from("/home/user/archive.zip")));
        let loc = Location::Archive {
            archive_path: archive_loc.clone(),
            inner_path: PathBuf::from(""),
        };
        let parent = loc.parent();
        // When inner_path is empty/root, parent should return the parent directory of the archive
        assert_eq!(parent, Some(Location::Local(PathBuf::from("/home/user"))));
    }

    #[test]
    fn test_archive_parent_from_root_file() {
        // Test navigating up from a file at the root of an archive
        let archive_loc = Box::new(Location::Local(PathBuf::from("/home/user/archive.zip")));
        let loc = Location::Archive {
            archive_path: archive_loc.clone(),
            inner_path: PathBuf::from("file.txt"),
        };
        let parent = loc.parent();
        // Parent of "file.txt" in archive should be the archive root (empty path)
        if let Some(Location::Archive { archive_path, inner_path }) = parent {
            assert_eq!(*archive_path, *archive_loc);
            assert_eq!(inner_path, PathBuf::from(""));
        } else {
            panic!("Expected Archive location with empty inner_path");
        }
    }

    #[test]
    fn test_local_join() {
        let loc = Location::Local(PathBuf::from("/home/user"));
        let joined = loc.join("documents");
        assert_eq!(joined, Location::Local(PathBuf::from("/home/user/documents")));
    }

    #[test]
    fn test_ssh_join() {
        let loc = Location::Ssh {
            host: "example.com".to_string(),
            port: 22,
            path: PathBuf::from("/var/www"),
        };
        let joined = loc.join("html");
        assert_eq!(
            joined,
            Location::Ssh {
                host: "example.com".to_string(),
                port: 22,
                path: PathBuf::from("/var/www/html"),
            }
        );
    }

    #[test]
    fn test_cloud_join() {
        let loc = Location::Cloud {
            provider: "s3".to_string(),
            bucket: "my-bucket".to_string(),
            path: PathBuf::from("folder"),
        };
        let joined = loc.join("file.txt");
        assert_eq!(
            joined,
            Location::Cloud {
                provider: "s3".to_string(),
                bucket: "my-bucket".to_string(),
                path: PathBuf::from("folder/file.txt"),
            }
        );
    }

    #[test]
    fn test_archive_join() {
        let archive_loc = Box::new(Location::Local(PathBuf::from("/home/user/archive.zip")));
        let loc = Location::Archive {
            archive_path: archive_loc.clone(),
            inner_path: PathBuf::from("folder"),
        };
        let joined = loc.join("file.txt");
        assert_eq!(
            joined,
            Location::Archive {
                archive_path: archive_loc,
                inner_path: PathBuf::from("folder/file.txt"),
            }
        );
    }

    #[test]
    fn test_nested_archive() {
        let outer_archive = Box::new(Location::Local(PathBuf::from("/home/user/outer.zip")));
        let inner_archive = Box::new(Location::Archive {
            archive_path: outer_archive.clone(),
            inner_path: PathBuf::from("inner.zip"),
        });
        let loc = Location::Archive {
            archive_path: inner_archive,
            inner_path: PathBuf::from("file.txt"),
        };
        
        let display = loc.display_path();
        assert_eq!(display, "/home/user/outer.zip#inner.zip#file.txt");
    }

    #[test]
    fn test_location_equality() {
        let loc1 = Location::Local(PathBuf::from("/home/user"));
        let loc2 = Location::Local(PathBuf::from("/home/user"));
        assert_eq!(loc1, loc2);
    }

    #[test]
    fn test_location_clone() {
        let loc = Location::Ssh {
            host: "example.com".to_string(),
            port: 22,
            path: PathBuf::from("/var/www"),
        };
        let cloned = loc.clone();
        assert_eq!(loc, cloned);
    }
}
