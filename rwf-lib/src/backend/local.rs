//! Local filesystem backend implementation
//!
//! This module implements the FilesystemBackend trait for local filesystem operations.

use crate::backend::FilesystemBackend;
use crate::model::{Location, FileEntry};
use tokio_util::sync::CancellationToken;
use anyhow::{Result, Context, bail};
use std::path::Path;
use std::time::SystemTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Local filesystem backend
pub struct LocalFilesystemBackend {
    buffer_size: usize,
}

impl LocalFilesystemBackend {
    /// Create a new local filesystem backend with default buffer size
    pub fn new() -> Self {
        Self {
            buffer_size: 8192, // 8KB default buffer
        }
    }
    
    /// Create a new local filesystem backend with custom buffer size
    pub fn with_buffer_size(buffer_size: usize) -> Self {
        Self { buffer_size }
    }
}

impl Default for LocalFilesystemBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl FilesystemBackend for LocalFilesystemBackend {
    async fn read_directory(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>> {
        use tracing::{debug, warn};
        
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };
        
        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }
        
        debug!("Reading directory: {}", location.display_path());
        
        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(path)
            .await
            .context("Failed to read directory")?;
        
        while let Some(entry) = read_dir.next_entry().await? {
            // Check cancellation periodically
            if cancel_token.is_cancelled() {
                bail!("Operation cancelled");
            }

            let metadata = entry.metadata().await?;
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Derive is_hidden from already-fetched metadata — avoids a second syscall per entry on Windows
            #[cfg(target_os = "windows")]
            let is_hidden = {
                use std::os::windows::fs::MetadataExt;
                const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
                (metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN) != 0
            };
            #[cfg(not(target_os = "windows"))]
            let is_hidden = name.starts_with('.');

            let file_entry = FileEntry {
                name,
                location: Location::Local(entry_path),
                size: metadata.len(),
                is_dir: metadata.is_dir(),
                is_hidden,
                modified: metadata.modified().unwrap_or_else(|_| SystemTime::now()),
                marked: false,
                calculated_size: None,
            };

            entries.push(file_entry);
        }
        
        debug!("Found {} entries in {}", entries.len(), location.display_path());
        
        // Special warning for C:\ with 0 entries
        if location.display_path() == "C:\\" && entries.is_empty() {
            warn!("C:\\ returned 0 entries - this may indicate a permissions issue");
        }
        
        Ok(entries)
    }
    
    async fn copy_file(
        &self,
        source: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let (src_path, dest_path) = match (source, dest) {
            (Location::Local(src), Location::Local(dst)) => (src, dst),
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };
        
        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }
        
        // Determine destination path
        let final_dest = if dest_path.is_dir() {
            dest_path.join(src_path.file_name().context("Invalid source path")?)
        } else {
            dest_path.clone()
        };
        
        // Check if source is a directory
        let metadata = tokio::fs::metadata(src_path)
            .await
            .context("Failed to read source metadata")?;
        
        if metadata.is_dir() {
            self.copy_directory(src_path, &final_dest, cancel_token).await?;
        } else {
            self.copy_single_file(src_path, &final_dest, cancel_token).await?;
        }
        
        Ok(())
    }
    
    async fn move_file(
        &self,
        source: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let (src_path, dest_path) = match (source, dest) {
            (Location::Local(src), Location::Local(dst)) => (src, dst),
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };
        
        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }
        
        // Determine destination path
        let final_dest = if dest_path.is_dir() {
            dest_path.join(src_path.file_name().context("Invalid source path")?)
        } else {
            dest_path.clone()
        };
        
        // Try atomic rename first (works if on same filesystem)
        match tokio::fs::rename(src_path, &final_dest).await {
            Ok(_) => Ok(()),
            Err(_) => {
                // If rename fails (cross-filesystem), copy then delete
                self.copy_file(source, dest, cancel_token).await?;
                self.delete_file(source, cancel_token).await?;
                Ok(())
            }
        }
    }
    
    async fn delete_file(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };
        
        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }
        
        let metadata = tokio::fs::metadata(path)
            .await
            .context("Failed to read file metadata")?;
        
        if metadata.is_dir() {
            tokio::fs::remove_dir_all(path)
                .await
                .context("Failed to delete directory")?;
        } else {
            tokio::fs::remove_file(path)
                .await
                .context("Failed to delete file")?;
        }
        
        Ok(())
    }
    
    async fn rename_file(
        &self,
        from: &Location,
        to: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let (from_path, to_path) = match (from, to) {
            (Location::Local(from), Location::Local(to)) => (from, to),
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };
        
        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }
        
        // Check if destination already exists
        if to_path.exists() {
            bail!("Destination file already exists");
        }
        
        // Validate filename (check for invalid characters)
        if let Some(filename) = to_path.file_name() {
            let filename_str = filename.to_string_lossy();
            if filename_str.contains(['<', '>', ':', '"', '|', '?', '*']) {
                bail!("Invalid characters in filename");
            }
        }
        
        tokio::fs::rename(from_path, to_path)
            .await
            .context("Failed to rename file")?;
        
        Ok(())
    }
    
    async fn create_directory(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };
        
        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }
        
        // Check if directory already exists
        if path.exists() {
            bail!("Directory already exists");
        }
        
        // Validate directory name
        if let Some(dirname) = path.file_name() {
            let dirname_str = dirname.to_string_lossy();
            if dirname_str.contains(['<', '>', ':', '"', '|', '?', '*']) {
                bail!("Invalid characters in directory name");
            }
        }
        
        tokio::fs::create_dir_all(path)
            .await
            .context("Failed to create directory")?;
        
        Ok(())
    }
    
    async fn calculate_directory_size(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<u64> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };
        
        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }
        
        self.calculate_dir_size_recursive(path, cancel_token).await
    }
    
    async fn calculate_directory_size_with_progress(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
        progress_callback: Box<dyn Fn(u64, u64) + Send + Sync>,
    ) -> Result<u64> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };
        
        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }
        
        let items_processed = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let current_size = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        
        self.calculate_dir_size_recursive_with_progress(
            path,
            cancel_token,
            items_processed.clone(),
            current_size.clone(),
            &*progress_callback,
        )
        .await
    }
    
    async fn read_file_content(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<u8>> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };
        
        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }
        
        let mut file = tokio::fs::File::open(path)
            .await
            .context("Failed to open file")?;
        
        let mut contents = Vec::new();
        let mut buffer = vec![0u8; self.buffer_size];
        
        loop {
            // Check cancellation periodically
            if cancel_token.is_cancelled() {
                bail!("Operation cancelled");
            }
            
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            
            contents.extend_from_slice(&buffer[..n]);
        }
        
        Ok(contents)
    }

    async fn get_entry(&self, location: &Location) -> Result<FileEntry> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        let metadata = tokio::fs::metadata(path)
            .await
            .context("Failed to read metadata")?;
        
        let name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        Ok(FileEntry {
            name,
            location: location.clone(),
            size: metadata.len(),
            is_dir: metadata.is_dir(),
            is_hidden: is_hidden(path),
            modified: metadata.modified().unwrap_or_else(|_| SystemTime::now()),
            marked: false,
            calculated_size: None,
        })
    }
}

// Private helper methods
impl LocalFilesystemBackend {
    /// Copy a single file with progress reporting
    async fn copy_single_file(
        &self,
        src: &Path,
        dest: &Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let mut src_file = tokio::fs::File::open(src)
            .await
            .context("Failed to open source file")?;
        
        let mut dest_file = tokio::fs::File::create(dest)
            .await
            .context("Failed to create destination file")?;
        
        let mut buffer = vec![0u8; self.buffer_size];
        
        loop {
            // Check cancellation periodically
            if cancel_token.is_cancelled() {
                bail!("Operation cancelled");
            }
            
            let n = src_file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            
            dest_file.write_all(&buffer[..n]).await?;
        }
        
        // Flush to ensure all data is written
        dest_file.flush().await?;
        
        Ok(())
    }
    
    /// Copy a directory recursively
    fn copy_directory<'a>(
        &'a self,
        src: &'a Path,
        dest: &'a Path,
        cancel_token: &'a CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            // Create destination directory
            tokio::fs::create_dir_all(dest)
                .await
                .context("Failed to create destination directory")?;
            
            let mut read_dir = tokio::fs::read_dir(src)
                .await
                .context("Failed to read source directory")?;
            
            while let Some(entry) = read_dir.next_entry().await? {
                // Check cancellation periodically
                if cancel_token.is_cancelled() {
                    bail!("Operation cancelled");
                }
                
                let src_path = entry.path();
                let dest_path = dest.join(entry.file_name());
                
                let metadata = entry.metadata().await?;
                
                if metadata.is_dir() {
                    self.copy_directory(&src_path, &dest_path, cancel_token).await?;
                } else {
                    self.copy_single_file(&src_path, &dest_path, cancel_token).await?;
                }
            }
            
            Ok(())
        })
    }
    
    /// Calculate directory size recursively
    fn calculate_dir_size_recursive<'a>(
        &'a self,
        path: &'a Path,
        cancel_token: &'a CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move {
            let mut total = 0u64;
            
            let mut read_dir = tokio::fs::read_dir(path)
                .await
                .context("Failed to read directory")?;
            
            while let Some(entry) = read_dir.next_entry().await? {
                // Check cancellation periodically
                if cancel_token.is_cancelled() {
                    bail!("Operation cancelled");
                }
                
                let metadata = entry.metadata().await?;
                
                if metadata.is_dir() {
                    total += self.calculate_dir_size_recursive(&entry.path(), cancel_token).await?;
                } else {
                    total += metadata.len();
                }
            }
            
            Ok(total)
        })
    }
    
    /// Calculate directory size recursively with progress reporting
    fn calculate_dir_size_recursive_with_progress<'a>(
        &'a self,
        path: &'a Path,
        cancel_token: &'a CancellationToken,
        items_processed: std::sync::Arc<std::sync::atomic::AtomicU64>,
        current_size: std::sync::Arc<std::sync::atomic::AtomicU64>,
        progress_callback: &'a (dyn Fn(u64, u64) + Send + Sync),
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move {
            let mut total = 0u64;
            
            let mut read_dir = tokio::fs::read_dir(path)
                .await
                .context("Failed to read directory")?;
            
            while let Some(entry) = read_dir.next_entry().await? {
                // Check cancellation periodically
                if cancel_token.is_cancelled() {
                    bail!("Operation cancelled");
                }
                
                let metadata = entry.metadata().await?;
                
                if metadata.is_dir() {
                    total += self.calculate_dir_size_recursive_with_progress(
                        &entry.path(),
                        cancel_token,
                        items_processed.clone(),
                        current_size.clone(),
                        progress_callback,
                    )
                    .await?;
                } else {
                    total += metadata.len();
                }
                
                // Update progress counters
                let items = items_processed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                let size = current_size.fetch_add(metadata.len(), std::sync::atomic::Ordering::Relaxed) + metadata.len();
                
                // Report progress every 100 items
                if items.is_multiple_of(100) {
                    progress_callback(items, size);
                }
            }
            
            Ok(total)
        })
    }
}

/// Check if a file is hidden
#[cfg(target_os = "windows")]
fn is_hidden(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    if let Ok(metadata) = path.metadata() {
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        (metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN) != 0
    } else {
        false
    }
}

/// Check if a file is hidden (Unix-like systems)
#[cfg(not(target_os = "windows"))]
fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_read_directory() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create some test files
        tokio::fs::write(temp_path.join("file1.txt"), b"content1").await.unwrap();
        tokio::fs::write(temp_path.join("file2.txt"), b"content2").await.unwrap();
        tokio::fs::create_dir(temp_path.join("subdir")).await.unwrap();
        
        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(temp_path.to_path_buf());
        let cancel_token = CancellationToken::new();
        
        let entries = backend.read_directory(&location, &cancel_token).await.unwrap();
        
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().any(|e| e.name == "file1.txt"));
        assert!(entries.iter().any(|e| e.name == "file2.txt"));
        assert!(entries.iter().any(|e| e.name == "subdir" && e.is_dir));
    }
    
    #[tokio::test]
    async fn test_copy_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        let src_path = temp_path.join("source.txt");
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();
        
        tokio::fs::write(&src_path, b"test content").await.unwrap();
        
        let backend = LocalFilesystemBackend::new();
        let src_location = Location::Local(src_path.clone());
        let dest_location = Location::Local(dest_dir.clone());
        let cancel_token = CancellationToken::new();
        
        backend.copy_file(&src_location, &dest_location, &cancel_token).await.unwrap();
        
        let dest_file = dest_dir.join("source.txt");
        assert!(dest_file.exists());
        let content = tokio::fs::read(&dest_file).await.unwrap();
        assert_eq!(content, b"test content");
    }
    
    #[tokio::test]
    async fn test_move_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        let src_path = temp_path.join("source.txt");
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();
        
        tokio::fs::write(&src_path, b"test content").await.unwrap();
        
        let backend = LocalFilesystemBackend::new();
        let src_location = Location::Local(src_path.clone());
        let dest_location = Location::Local(dest_dir.clone());
        let cancel_token = CancellationToken::new();
        
        backend.move_file(&src_location, &dest_location, &cancel_token).await.unwrap();
        
        assert!(!src_path.exists());
        let dest_file = dest_dir.join("source.txt");
        assert!(dest_file.exists());
        let content = tokio::fs::read(&dest_file).await.unwrap();
        assert_eq!(content, b"test content");
    }
    
    #[tokio::test]
    async fn test_delete_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        let file_path = temp_path.join("to_delete.txt");
        tokio::fs::write(&file_path, b"delete me").await.unwrap();
        
        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(file_path.clone());
        let cancel_token = CancellationToken::new();
        
        backend.delete_file(&location, &cancel_token).await.unwrap();
        
        assert!(!file_path.exists());
    }
    
    #[tokio::test]
    async fn test_rename_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        let old_path = temp_path.join("old_name.txt");
        let new_path = temp_path.join("new_name.txt");
        tokio::fs::write(&old_path, b"content").await.unwrap();
        
        let backend = LocalFilesystemBackend::new();
        let from_location = Location::Local(old_path.clone());
        let to_location = Location::Local(new_path.clone());
        let cancel_token = CancellationToken::new();
        
        backend.rename_file(&from_location, &to_location, &cancel_token).await.unwrap();
        
        assert!(!old_path.exists());
        assert!(new_path.exists());
    }
    
    #[tokio::test]
    async fn test_create_directory() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        let new_dir = temp_path.join("new_directory");
        
        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(new_dir.clone());
        let cancel_token = CancellationToken::new();
        
        backend.create_directory(&location, &cancel_token).await.unwrap();
        
        assert!(new_dir.exists());
        assert!(new_dir.is_dir());
    }
    
    #[tokio::test]
    async fn test_calculate_directory_size() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create files with known sizes
        tokio::fs::write(temp_path.join("file1.txt"), vec![0u8; 100]).await.unwrap();
        tokio::fs::write(temp_path.join("file2.txt"), vec![0u8; 200]).await.unwrap();
        
        let subdir = temp_path.join("subdir");
        tokio::fs::create_dir(&subdir).await.unwrap();
        tokio::fs::write(subdir.join("file3.txt"), vec![0u8; 300]).await.unwrap();
        
        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(temp_path.to_path_buf());
        let cancel_token = CancellationToken::new();
        
        let size = backend.calculate_directory_size(&location, &cancel_token).await.unwrap();
        
        assert_eq!(size, 600); // 100 + 200 + 300
    }
    
    #[tokio::test]
    async fn test_calculate_directory_size_with_cancellation() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create a directory structure with many files
        for i in 0..10 {
            let subdir = temp_path.join(format!("subdir{}", i));
            tokio::fs::create_dir(&subdir).await.unwrap();
            for j in 0..10 {
                tokio::fs::write(subdir.join(format!("file{}.txt", j)), vec![0u8; 100])
                    .await
                    .unwrap();
            }
        }
        
        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(temp_path.to_path_buf());
        let cancel_token = CancellationToken::new();
        
        // Cancel the token immediately
        cancel_token.cancel();
        
        // The operation should fail with cancellation error
        let result = backend.calculate_directory_size(&location, &cancel_token).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }
    
    #[tokio::test]
    async fn test_calculate_directory_size_with_progress() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create a directory structure with many files to trigger progress reporting
        for i in 0..5 {
            let subdir = temp_path.join(format!("subdir{}", i));
            tokio::fs::create_dir(&subdir).await.unwrap();
            for j in 0..25 {
                tokio::fs::write(subdir.join(format!("file{}.txt", j)), vec![0u8; 100])
                    .await
                    .unwrap();
            }
        }
        
        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(temp_path.to_path_buf());
        let cancel_token = CancellationToken::new();
        
        // Track progress updates
        let progress_updates = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let progress_updates_clone = progress_updates.clone();
        
        let size = backend
            .calculate_directory_size_with_progress(&location, &cancel_token, Box::new(move |items, size| {
                progress_updates_clone.lock().unwrap().push((items, size));
            }))
            .await
            .unwrap();
        
        // Verify the total size is correct (5 subdirs * 25 files * 100 bytes = 12500)
        assert_eq!(size, 12500);
        
        // Verify we received progress updates
        let updates = progress_updates.lock().unwrap();
        assert!(!updates.is_empty(), "Should have received progress updates");
        
        // Verify progress updates are increasing
        for i in 1..updates.len() {
            assert!(updates[i].0 >= updates[i - 1].0, "Items should be increasing");
            assert!(updates[i].1 >= updates[i - 1].1, "Size should be increasing");
        }
    }
    
    #[tokio::test]
    async fn test_cancellation() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        let src_path = temp_path.join("source.txt");
        tokio::fs::write(&src_path, vec![0u8; 1000]).await.unwrap();
        
        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(src_path);
        let cancel_token = CancellationToken::new();
        
        // Cancel immediately
        cancel_token.cancel();
        
        let result = backend.read_file_content(&location, &cancel_token).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }
}
