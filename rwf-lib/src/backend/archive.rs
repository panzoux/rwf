//! Archive handling for browsing and extracting compressed files
//!
//! This module provides the ArchiveHandler trait and implementations
//! for different archive formats (initially .zip).

use crate::model::{Location, FileEntry};
use tokio_util::sync::CancellationToken;
use anyhow::Result;
use std::io::Write;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// Trait for archive operations
#[async_trait::async_trait]
pub trait ArchiveHandler: Send + Sync {
    /// List entries in an archive at a specific path
    /// 
    /// For Location::Archive, lists the direct children at inner_path.
    /// Returns FileEntry items with Archive locations.
    async fn list_entries(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>>;
    
    /// Extract a single file from an archive
    async fn extract_file(
        &self,
        source: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()>;
    
    /// Extract entire archive to destination
    async fn extract_all(
        &self,
        archive: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()>;
    
    /// Create archive from source files
    async fn create_archive(
        &self,
        sources: &[Location],
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()>;
    
    /// Check if a file is a supported archive format
    fn is_archive(&self, filename: &str) -> bool;
}

/// ZIP archive handler implementation
pub struct ZipArchiveHandler;

impl ZipArchiveHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ArchiveHandler for ZipArchiveHandler {
    async fn list_entries(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>> {
        match location {
            Location::Archive { archive_path, inner_path } => {
                // Only support local archives for now
                match archive_path.as_ref() {
                    Location::Local(path) => {
                        self.list_zip_entries(path, inner_path, cancel_token).await
                    }
                    _ => anyhow::bail!("Only local archives are supported"),
                }
            }
            _ => anyhow::bail!("Expected Archive location"),
        }
    }
    
    async fn extract_file(
        &self,
        source: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        match (source, dest) {
            (Location::Archive { archive_path, inner_path }, Location::Local(dest_path)) => {
                match archive_path.as_ref() {
                    Location::Local(archive_file) => {
                        self.extract_zip_file(archive_file, inner_path, dest_path, cancel_token).await
                    }
                    _ => anyhow::bail!("Only local archives are supported"),
                }
            }
            _ => anyhow::bail!("Invalid source/dest combination for archive extraction"),
        }
    }
    
    async fn extract_all(
        &self,
        archive: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        match (archive, dest) {
            (Location::Local(archive_path), Location::Local(dest_path)) => {
                self.extract_zip_all(archive_path, dest_path, cancel_token).await
            }
            (Location::Archive { archive_path, inner_path }, Location::Local(dest_path)) => {
                // Extract a subdirectory from archive
                match archive_path.as_ref() {
                    Location::Local(archive_file) => {
                        self.extract_zip_subdir(archive_file, inner_path, dest_path, cancel_token).await
                    }
                    _ => anyhow::bail!("Only local archives are supported"),
                }
            }
            _ => anyhow::bail!("Invalid archive/dest combination"),
        }
    }
    
    async fn create_archive(
        &self,
        sources: &[Location],
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        match dest {
            Location::Local(dest_path) => {
                self.create_zip(sources, dest_path, cancel_token).await
            }
            _ => anyhow::bail!("Archive destination must be local"),
        }
    }
    
    fn is_archive(&self, filename: &str) -> bool {
        filename.to_lowercase().ends_with(".zip")
    }
}

impl ZipArchiveHandler {
    /// List entries in a ZIP archive at a specific inner path
    async fn list_zip_entries(
        &self,
        archive_path: &std::path::Path,
        inner_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>> {
        use std::fs::File;
        use zip::ZipArchive;
        use std::time::SystemTime;
        
        if cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }
        
        let file = File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)?;
        
        let mut entries = Vec::new();
        let inner_str = inner_path.to_string_lossy();
        let prefix = if inner_str.is_empty() {
            String::new()
        } else {
            format!("{}/", inner_str.trim_end_matches('/'))
        };
        
        // Collect all entries that are direct children of inner_path
        let mut seen_names = std::collections::HashSet::new();
        
        for i in 0..archive.len() {
            if cancel_token.is_cancelled() {
                anyhow::bail!("Operation cancelled");
            }
            
            let file = archive.by_index(i)?;
            let file_path = file.name();
            
            // Check if this file is under our prefix
            if file_path.starts_with(&prefix) {
                let relative = &file_path[prefix.len()..];
                
                // Skip empty paths
                if relative.is_empty() {
                    continue;
                }
                
                // Get the first component (direct child)
                let first_component = if let Some(slash_pos) = relative.find('/') {
                    &relative[..slash_pos]
                } else {
                    relative
                };
                
                // Skip if we've already seen this entry
                if !seen_names.insert(first_component.to_string()) {
                    continue;
                }
                
                // Determine if this is a directory
                let is_dir = relative.contains('/') || file.is_dir();
                
                let entry_inner_path = if prefix.is_empty() {
                    std::path::PathBuf::from(first_component)
                } else {
                    inner_path.join(first_component)
                };
                
                let entry = FileEntry {
                    name: first_component.to_string(),
                    location: Location::Archive {
                        archive_path: Box::new(Location::Local(archive_path.to_path_buf())),
                        inner_path: entry_inner_path,
                    },
                    size: if is_dir { 0 } else { file.size() },
                    is_dir,
                    is_hidden: first_component.starts_with('.'),
                    modified: SystemTime::now(), // TODO: Extract proper timestamp from zip
                    marked: false,
                    calculated_size: None,
                };
                
                entries.push(entry);
            }
        }
        
        Ok(entries)
    }
    
    /// Extract a single file from ZIP archive
    async fn extract_zip_file(
        &self,
        archive_path: &std::path::Path,
        inner_path: &std::path::Path,
        dest_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        use std::fs::File;
        use zip::ZipArchive;
        
        if cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }
        
        let file = File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)?;
        
        let inner_str = inner_path.to_string_lossy();
        let mut zip_file = archive.by_name(&inner_str)?;
        
        if zip_file.is_dir() {
            std::fs::create_dir_all(dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            
            let mut outfile = File::create(dest_path)?;
            std::io::copy(&mut zip_file, &mut outfile)?;
        }
        
        Ok(())
    }
    
    /// Extract entire ZIP archive
    async fn extract_zip_all(
        &self,
        archive_path: &std::path::Path,
        dest_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        use std::fs::File;
        use zip::ZipArchive;
        
        let file = File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)?;
        
        for i in 0..archive.len() {
            if cancel_token.is_cancelled() {
                anyhow::bail!("Operation cancelled");
            }
            
            let mut file = archive.by_index(i)?;
            let outpath = dest_path.join(file.name());
            
            // Check if it's a directory (either by is_dir() or trailing slash)
            if file.is_dir() || file.name().ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                
                let mut outfile = File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }
        
        Ok(())
    }
    
    /// Extract a subdirectory from ZIP archive
    async fn extract_zip_subdir(
        &self,
        archive_path: &std::path::Path,
        inner_path: &std::path::Path,
        dest_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        use std::fs::File;
        use zip::ZipArchive;
        
        let file = File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)?;
        
        let inner_str = inner_path.to_string_lossy();
        let prefix = format!("{}/", inner_str.trim_end_matches('/'));
        
        for i in 0..archive.len() {
            if cancel_token.is_cancelled() {
                anyhow::bail!("Operation cancelled");
            }
            
            let mut file = archive.by_index(i)?;
            let file_path = file.name();
            
            if file_path.starts_with(&prefix) {
                let relative = &file_path[prefix.len()..];
                if relative.is_empty() {
                    continue;
                }
                
                let outpath = dest_path.join(relative);
                
                if file.is_dir() {
                    std::fs::create_dir_all(&outpath)?;
                } else {
                    if let Some(parent) = outpath.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    
                    let mut outfile = File::create(&outpath)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Create a ZIP archive from source files
    async fn create_zip(
        &self,
        sources: &[Location],
        dest_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        use std::fs::File;
        use std::io::Write;
        use tracing::debug;

        debug!("Creating ZIP archive at: {:?}", dest_path);
        let file = File::create(dest_path)?;
        debug!("File created successfully");
        
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        debug!("Adding {} sources to archive", sources.len());
        for source in sources {
            if cancel_token.is_cancelled() {
                debug!("Archive creation cancelled");
                anyhow::bail!("Operation cancelled");
            }

            match source {
                Location::Local(path) => {
                    debug!("Processing source: {:?}", path);
                    if path.is_dir() {
                        debug!("Source is a directory");
                        // Get the directory name to use as the base in the archive
                        let dir_name = path.file_name()
                            .and_then(|n| n.to_str())
                            .ok_or_else(|| anyhow::anyhow!("Invalid directory name"))?;

                        // Add the directory entry itself
                        zip.add_directory(format!("{}/", dir_name), options)?;
                        debug!("Added directory entry: {}", dir_name);

                        // Add contents with the directory as base
                        self.add_dir_contents_to_zip(&mut zip, path, dir_name, &options, cancel_token).await?;
                        debug!("Added directory contents");
                    } else {
                        debug!("Source is a file");
                        let name = path.file_name()
                            .and_then(|n| n.to_str())
                            .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
                        self.add_file_to_zip(&mut zip, path, name, &options).await?;
                        debug!("Added file to archive: {}", name);
                    }
                }
                _ => anyhow::bail!("Only local files can be added to archives"),
            }
        }

        // Finish writing and get the underlying file
        debug!("Finishing ZIP archive...");
        let mut file = zip.finish()?;
        debug!("ZIP finished, flushing...");
        // Explicitly flush and sync to ensure data is written to disk
        file.flush()?;
        file.sync_all()?;
        debug!("ZIP archive created and synced successfully at: {:?}", dest_path);
        
        Ok(())
    }
    
    /// Add a file to ZIP archive
    async fn add_file_to_zip<W: std::io::Write + std::io::Seek>(
        &self,
        zip: &mut ZipWriter<W>,
        path: &std::path::Path,
        name: &str,
        options: &SimpleFileOptions,
    ) -> Result<()> {
        use std::fs::File;
        use std::io::Read;
        
        zip.start_file(name, *options)?;
        
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        zip.write_all(&buffer)?;
        
        Ok(())
    }
    
    /// Add a directory recursively to ZIP archive
    #[allow(dead_code)]
    fn add_dir_to_zip<'a, W: std::io::Write + std::io::Seek + Send + 'a>(
        &'a self,
        zip: &'a mut ZipWriter<W>,
        dir_path: &'a std::path::Path,
        base_path: &'a std::path::Path,
        options: &'a SimpleFileOptions,
        cancel_token: &'a CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            for entry in std::fs::read_dir(dir_path)? {
                if cancel_token.is_cancelled() {
                    anyhow::bail!("Operation cancelled");
                }
                
                let entry = entry?;
                let path = entry.path();
                let name = path.strip_prefix(base_path)?
                    .to_string_lossy()
                    .to_string();
                
                if path.is_dir() {
                    // Add directory entry with trailing slash
                    let dir_name = if name.ends_with('/') {
                        name.clone()
                    } else {
                        format!("{}/", name)
                    };
                    zip.add_directory(&dir_name, *options)?;
                    // Recursively add contents
                    self.add_dir_to_zip(zip, &path, base_path, options, cancel_token).await?;
                } else {
                    self.add_file_to_zip(zip, &path, &name, options).await?;
                }
            }
            
            Ok(())
        })
    }
    
    /// Add directory contents to ZIP archive with a prefix
    fn add_dir_contents_to_zip<'a, W: std::io::Write + std::io::Seek + Send + 'a>(
        &'a self,
        zip: &'a mut ZipWriter<W>,
        dir_path: &'a std::path::Path,
        prefix: &'a str,
        options: &'a SimpleFileOptions,
        cancel_token: &'a CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            for entry in std::fs::read_dir(dir_path)? {
                if cancel_token.is_cancelled() {
                    anyhow::bail!("Operation cancelled");
                }
                
                let entry = entry?;
                let path = entry.path();
                let file_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
                let name = format!("{}/{}", prefix, file_name);
                
                if path.is_dir() {
                    // Add directory entry with trailing slash
                    zip.add_directory(format!("{}/", name), *options)?;
                    // Recursively add contents
                    self.add_dir_contents_to_zip(zip, &path, &name, options, cancel_token).await?;
                } else {
                    self.add_file_to_zip(zip, &path, &name, options).await?;
                }
            }
            
            Ok(())
        })
    }
}

impl Default for ZipArchiveHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock archive handler for testing
#[cfg(test)]
pub struct MockArchiveHandler;

#[cfg(test)]
#[async_trait::async_trait]
impl ArchiveHandler for MockArchiveHandler {
    async fn list_entries(
        &self,
        _location: &Location,
        _cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>> {
        Ok(Vec::new())
    }

    async fn extract_file(
        &self,
        _source: &Location,
        _dest: &Location,
        _cancel_token: &CancellationToken,
    ) -> Result<()> {
        Ok(())
    }

    async fn extract_all(
        &self,
        _archive: &Location,
        _dest: &Location,
        _cancel_token: &CancellationToken,
    ) -> Result<()> {
        Ok(())
    }

    async fn create_archive(
        &self,
        _sources: &[Location],
        _dest: &Location,
        _cancel_token: &CancellationToken,
    ) -> Result<()> {
        Ok(())
    }
    
    fn is_archive(&self, filename: &str) -> bool {
        filename.ends_with(".zip")
    }
}
