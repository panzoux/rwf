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

// ──────────────────────────────────────────────────────────────────────────────
// SevenZArchiveHandler
// ──────────────────────────────────────────────────────────────────────────────

/// 7z archive handler implementation using the sevenz-rust crate
pub struct SevenZArchiveHandler;

impl SevenZArchiveHandler {
    pub fn new() -> Self { Self }
}

impl Default for SevenZArchiveHandler {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl ArchiveHandler for SevenZArchiveHandler {
    async fn list_entries(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>> {
        match location {
            Location::Archive { archive_path, inner_path } => {
                match archive_path.as_ref() {
                    Location::Local(path) => {
                        self.list_sevenz_entries(path, inner_path, cancel_token).await
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
                        self.extract_sevenz_file(archive_file, inner_path, dest_path, cancel_token).await
                    }
                    _ => anyhow::bail!("Only local archives are supported"),
                }
            }
            _ => anyhow::bail!("Invalid source/dest combination for 7z extraction"),
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
                if cancel_token.is_cancelled() {
                    anyhow::bail!("Operation cancelled");
                }
                std::fs::create_dir_all(dest_path)?;
                sevenz_rust::decompress_file(archive_path, dest_path)
                    .map_err(|e| anyhow::anyhow!("7z extraction failed: {}", e))
            }
            (Location::Archive { archive_path, inner_path }, Location::Local(dest_path)) => {
                match archive_path.as_ref() {
                    Location::Local(archive_file) => {
                        self.extract_sevenz_subdir(archive_file, inner_path, dest_path, cancel_token).await
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
                self.create_sevenz(sources, dest_path, cancel_token).await
            }
            _ => anyhow::bail!("Archive destination must be local"),
        }
    }

    fn is_archive(&self, filename: &str) -> bool {
        filename.to_lowercase().ends_with(".7z")
    }
}

impl SevenZArchiveHandler {
    async fn list_sevenz_entries(
        &self,
        archive_path: &std::path::Path,
        inner_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>> {
        use std::time::SystemTime;

        if cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }

        let reader = sevenz_rust::SevenZReader::open(archive_path, sevenz_rust::Password::empty())
            .map_err(|e| anyhow::anyhow!("Failed to open 7z archive: {}", e))?;

        let inner_str = inner_path.to_string_lossy();
        let prefix = if inner_str.is_empty() {
            String::new()
        } else {
            format!("{}/", inner_str.trim_end_matches('/'))
        };

        let mut entries = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        for file in &reader.archive().files {
            if cancel_token.is_cancelled() {
                anyhow::bail!("Operation cancelled");
            }

            let file_path = file.name.replace('\\', "/");
            if !file_path.starts_with(&prefix) {
                continue;
            }

            let relative = &file_path[prefix.len()..];
            if relative.is_empty() {
                continue;
            }

            let first_component = if let Some(slash_pos) = relative.find('/') {
                &relative[..slash_pos]
            } else {
                relative.trim_end_matches('/')
            };

            if first_component.is_empty() || !seen_names.insert(first_component.to_string()) {
                continue;
            }

            let is_dir = relative.contains('/') || file.is_directory;
            let entry_inner_path = if prefix.is_empty() {
                std::path::PathBuf::from(first_component)
            } else {
                inner_path.join(first_component)
            };

            entries.push(FileEntry {
                name: first_component.to_string(),
                location: Location::Archive {
                    archive_path: Box::new(Location::Local(archive_path.to_path_buf())),
                    inner_path: entry_inner_path,
                },
                size: if is_dir { 0 } else { file.size },
                is_dir,
                is_hidden: first_component.starts_with('.'),
                modified: SystemTime::now(),
                marked: false,
                calculated_size: None,
            });
        }

        Ok(entries)
    }

    async fn extract_sevenz_file(
        &self,
        archive_path: &std::path::Path,
        inner_path: &std::path::Path,
        dest_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        if cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }

        let inner_str = inner_path.to_string_lossy().replace('\\', "/");
        let dest_path = dest_path.to_path_buf();
        let mut found = false;
        let mut io_error: Option<std::io::Error> = None;

        let mut reader = sevenz_rust::SevenZReader::open(archive_path, sevenz_rust::Password::empty())
            .map_err(|e| anyhow::anyhow!("Failed to open 7z archive: {}", e))?;

        reader.for_each_entries(|entry, source| {
            if entry.name.replace('\\', "/") != inner_str {
                return Ok(true);
            }
            let result = (|| -> std::io::Result<()> {
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut outfile = std::fs::File::create(&dest_path)?;
                std::io::copy(source, &mut outfile)?;
                Ok(())
            })();
            if let Err(e) = result {
                io_error = Some(e);
            }
            found = true;
            Ok(false)
        }).map_err(|e| anyhow::anyhow!("7z extraction error: {}", e))?;

        if let Some(e) = io_error {
            return Err(anyhow::Error::from(e));
        }
        if !found {
            anyhow::bail!("File not found in 7z archive: {}", inner_str);
        }
        Ok(())
    }

    async fn extract_sevenz_subdir(
        &self,
        archive_path: &std::path::Path,
        inner_path: &std::path::Path,
        dest_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        if cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }

        let inner_str = inner_path.to_string_lossy().replace('\\', "/");
        let prefix = format!("{}/", inner_str.trim_end_matches('/'));
        let dest_path = dest_path.to_path_buf();
        let mut io_error: Option<std::io::Error> = None;

        let mut reader = sevenz_rust::SevenZReader::open(archive_path, sevenz_rust::Password::empty())
            .map_err(|e| anyhow::anyhow!("Failed to open 7z archive: {}", e))?;

        reader.for_each_entries(|entry, source| {
            let entry_name = entry.name.replace('\\', "/");
            if !entry_name.starts_with(&prefix) {
                return Ok(true);
            }
            let relative = &entry_name[prefix.len()..];
            if relative.is_empty() {
                return Ok(true);
            }
            let outpath = dest_path.join(relative);
            let result = (|| -> std::io::Result<()> {
                if entry.is_directory {
                    std::fs::create_dir_all(&outpath)?;
                } else {
                    if let Some(parent) = outpath.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let mut outfile = std::fs::File::create(&outpath)?;
                    std::io::copy(source, &mut outfile)?;
                }
                Ok(())
            })();
            if let Err(e) = result {
                io_error = Some(e);
                return Ok(false);
            }
            Ok(true)
        }).map_err(|e| anyhow::anyhow!("7z extraction error: {}", e))?;

        if let Some(e) = io_error {
            return Err(anyhow::Error::from(e));
        }
        Ok(())
    }

    async fn create_sevenz(
        &self,
        sources: &[Location],
        dest_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let mut writer = sevenz_rust::SevenZWriter::create(dest_path)
            .map_err(|e| anyhow::anyhow!("Failed to create 7z archive: {}", e))?;

        for source in sources {
            if cancel_token.is_cancelled() {
                anyhow::bail!("Operation cancelled");
            }
            match source {
                Location::Local(path) => {
                    let name = path.file_name()
                        .and_then(|n| n.to_str())
                        .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
                    add_path_to_sevenz_writer(&mut writer, path, name, cancel_token)?;
                }
                _ => anyhow::bail!("Only local files can be added to archives"),
            }
        }

        writer.finish()
            .map_err(|e| anyhow::anyhow!("Failed to finalize 7z archive: {}", e))?;
        Ok(())
    }
}

/// Recursively add a file or directory to a SevenZWriter.
fn add_path_to_sevenz_writer<W: std::io::Write + std::io::Seek>(
    writer: &mut sevenz_rust::SevenZWriter<W>,
    path: &std::path::Path,
    arc_name: &str,
    cancel_token: &CancellationToken,
) -> Result<()> {
    if path.is_dir() {
        let mut dir_entry = sevenz_rust::SevenZArchiveEntry::default();
        dir_entry.name = format!("{}/", arc_name);
        dir_entry.is_directory = true;
        writer.push_archive_entry::<std::fs::File>(dir_entry, None)
            .map_err(|e| anyhow::anyhow!("7z dir entry error: {}", e))?;

        for entry in std::fs::read_dir(path)? {
            if cancel_token.is_cancelled() {
                anyhow::bail!("Operation cancelled");
            }
            let entry = entry?;
            let child_path = entry.path();
            let child_name = child_path.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
            let child_arc_name = format!("{}/{}", arc_name, child_name);
            add_path_to_sevenz_writer(writer, &child_path, &child_arc_name, cancel_token)?;
        }
    } else {
        let meta = std::fs::metadata(path)?;
        let mut file_entry = sevenz_rust::SevenZArchiveEntry::default();
        file_entry.name = arc_name.to_string();
        file_entry.has_stream = true;
        file_entry.size = meta.len();
        let file = std::fs::File::open(path)?;
        writer.push_archive_entry(file_entry, Some(file))
            .map_err(|e| anyhow::anyhow!("7z file entry error: {}", e))?;
    }
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// TarArchiveHandler
// ──────────────────────────────────────────────────────────────────────────────

enum TarCompression { None, Gz }

/// TAR/TGZ archive handler (.tar, .tgz, .tar.gz)
pub struct TarArchiveHandler;

impl TarArchiveHandler {
    pub fn new() -> Self { Self }

    fn compression(path: &std::path::Path) -> TarCompression {
        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        if name.ends_with(".tgz") || name.ends_with(".tar.gz") {
            TarCompression::Gz
        } else {
            TarCompression::None
        }
    }
}

impl Default for TarArchiveHandler {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl ArchiveHandler for TarArchiveHandler {
    async fn list_entries(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>> {
        match location {
            Location::Archive { archive_path, inner_path } => {
                match archive_path.as_ref() {
                    Location::Local(path) => {
                        self.list_tar_entries(path, inner_path, cancel_token).await
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
                        self.extract_tar_file(archive_file, inner_path, dest_path, cancel_token).await
                    }
                    _ => anyhow::bail!("Only local archives are supported"),
                }
            }
            _ => anyhow::bail!("Invalid source/dest combination for TAR extraction"),
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
                self.extract_tar_all(archive_path, dest_path, cancel_token).await
            }
            (Location::Archive { archive_path, inner_path }, Location::Local(dest_path)) => {
                match archive_path.as_ref() {
                    Location::Local(archive_file) => {
                        self.extract_tar_subdir(archive_file, inner_path, dest_path, cancel_token).await
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
                self.create_tar(sources, dest_path, cancel_token).await
            }
            _ => anyhow::bail!("Archive destination must be local"),
        }
    }

    fn is_archive(&self, filename: &str) -> bool {
        let name = filename.to_lowercase();
        name.ends_with(".tar") || name.ends_with(".tgz") || name.ends_with(".tar.gz")
    }
}

/// Collect direct children of `inner_path` from any tar reader.
fn collect_tar_direct_children<R: std::io::Read>(
    reader: R,
    archive_path: &std::path::Path,
    inner_path: &std::path::Path,
    cancel_token: &CancellationToken,
) -> Result<Vec<FileEntry>> {
    use std::time::SystemTime;

    let mut archive = tar::Archive::new(reader);
    let inner_str = inner_path.to_string_lossy();
    let prefix = if inner_str.is_empty() {
        String::new()
    } else {
        format!("{}/", inner_str.trim_end_matches('/'))
    };

    let mut seen = std::collections::HashSet::new();
    let mut entries = Vec::new();

    for entry_result in archive.entries()? {
        if cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }
        let entry = entry_result?;
        let raw = entry.path()?.to_string_lossy().replace('\\', "/");
        let raw = raw.as_str();
        let file_path = raw.strip_prefix("./").unwrap_or(raw);
        let file_path = file_path.trim_end_matches('/');
        if file_path.is_empty() || !file_path.starts_with(prefix.as_str()) {
            continue;
        }

        let relative = &file_path[prefix.len()..];
        if relative.is_empty() { continue; }

        let first = if let Some(pos) = relative.find('/') {
            &relative[..pos]
        } else {
            relative
        };
        if first.is_empty() || !seen.insert(first.to_string()) { continue; }

        let is_dir = relative.contains('/') || entry.header().entry_type().is_dir();
        let size = if is_dir { 0 } else { entry.header().size()? };
        let entry_inner = if prefix.is_empty() {
            std::path::PathBuf::from(first)
        } else {
            inner_path.join(first)
        };

        entries.push(FileEntry {
            name: first.to_string(),
            location: Location::Archive {
                archive_path: Box::new(Location::Local(archive_path.to_path_buf())),
                inner_path: entry_inner,
            },
            size,
            is_dir,
            is_hidden: first.starts_with('.'),
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        });
    }

    Ok(entries)
}

/// Extract a single named entry from any tar reader. Returns true if found.
fn extract_single_tar_entry<R: std::io::Read>(
    reader: R,
    inner_str: &str,
    dest_path: &std::path::Path,
    cancel_token: &CancellationToken,
) -> Result<bool> {
    let mut archive = tar::Archive::new(reader);
    for entry_result in archive.entries()? {
        if cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }
        let mut entry = entry_result?;
        let raw = entry.path()?.to_string_lossy().replace('\\', "/");
        let raw = raw.as_str();
        let file_path = raw.strip_prefix("./").unwrap_or(raw);
        let file_path = file_path.trim_end_matches('/');
        if file_path != inner_str { continue; }

        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(dest_path)?;
        std::io::copy(&mut entry, &mut out)?;
        return Ok(true);
    }
    Ok(false)
}

/// Extract entries under `prefix` from any tar reader to `dest_path`.
fn extract_tar_subdir_from_reader<R: std::io::Read>(
    reader: R,
    prefix: &str,
    dest_path: &std::path::Path,
    cancel_token: &CancellationToken,
) -> Result<()> {
    let mut archive = tar::Archive::new(reader);
    for entry_result in archive.entries()? {
        if cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }
        let mut entry = entry_result?;
        let raw = entry.path()?.to_string_lossy().replace('\\', "/");
        let raw = raw.as_str();
        let file_path = raw.strip_prefix("./").unwrap_or(raw);
        if !file_path.starts_with(prefix) { continue; }
        let relative = &file_path[prefix.len()..];
        if relative.is_empty() { continue; }

        let outpath = dest_path.join(relative);
        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() { std::fs::create_dir_all(p)?; }
            let mut out = std::fs::File::create(&outpath)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

/// Add a single source Location to any tar Builder.
fn add_to_tar_builder<W: std::io::Write>(
    builder: &mut tar::Builder<W>,
    source: &Location,
) -> Result<()> {
    match source {
        Location::Local(path) => {
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
            if path.is_dir() {
                builder.append_dir_all(name, path)?;
            } else {
                builder.append_path_with_name(path, name)?;
            }
            Ok(())
        }
        _ => anyhow::bail!("Only local files can be added to archives"),
    }
}

impl TarArchiveHandler {
    async fn list_tar_entries(
        &self,
        archive_path: &std::path::Path,
        inner_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>> {
        if cancel_token.is_cancelled() { anyhow::bail!("Operation cancelled"); }
        let file = std::fs::File::open(archive_path)?;
        match TarArchiveHandler::compression(archive_path) {
            TarCompression::Gz => {
                collect_tar_direct_children(
                    flate2::read::GzDecoder::new(file), archive_path, inner_path, cancel_token)
            }
            TarCompression::None => {
                collect_tar_direct_children(file, archive_path, inner_path, cancel_token)
            }
        }
    }

    async fn extract_tar_all(
        &self,
        archive_path: &std::path::Path,
        dest_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        if cancel_token.is_cancelled() { anyhow::bail!("Operation cancelled"); }
        std::fs::create_dir_all(dest_path)?;
        let file = std::fs::File::open(archive_path)?;
        match TarArchiveHandler::compression(archive_path) {
            TarCompression::Gz => {
                let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
                archive.unpack(dest_path).map_err(|e| anyhow::anyhow!("TAR extraction failed: {}", e))
            }
            TarCompression::None => {
                let mut archive = tar::Archive::new(file);
                archive.unpack(dest_path).map_err(|e| anyhow::anyhow!("TAR extraction failed: {}", e))
            }
        }
    }

    async fn extract_tar_file(
        &self,
        archive_path: &std::path::Path,
        inner_path: &std::path::Path,
        dest_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        if cancel_token.is_cancelled() { anyhow::bail!("Operation cancelled"); }
        let inner_str = inner_path.to_string_lossy().replace('\\', "/");
        let file = std::fs::File::open(archive_path)?;
        let found = match TarArchiveHandler::compression(archive_path) {
            TarCompression::Gz => {
                extract_single_tar_entry(
                    flate2::read::GzDecoder::new(file), &inner_str, dest_path, cancel_token)?
            }
            TarCompression::None => {
                extract_single_tar_entry(file, &inner_str, dest_path, cancel_token)?
            }
        };
        if !found { anyhow::bail!("File not found in TAR archive: {}", inner_str); }
        Ok(())
    }

    async fn extract_tar_subdir(
        &self,
        archive_path: &std::path::Path,
        inner_path: &std::path::Path,
        dest_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        if cancel_token.is_cancelled() { anyhow::bail!("Operation cancelled"); }
        let inner_str = inner_path.to_string_lossy().replace('\\', "/");
        let prefix = format!("{}/", inner_str.trim_end_matches('/'));
        let file = std::fs::File::open(archive_path)?;
        match TarArchiveHandler::compression(archive_path) {
            TarCompression::Gz => {
                extract_tar_subdir_from_reader(
                    flate2::read::GzDecoder::new(file), &prefix, dest_path, cancel_token)
            }
            TarCompression::None => {
                extract_tar_subdir_from_reader(file, &prefix, dest_path, cancel_token)
            }
        }
    }

    async fn create_tar(
        &self,
        sources: &[Location],
        dest_path: &std::path::Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let file = std::fs::File::create(dest_path)?;
        match TarArchiveHandler::compression(dest_path) {
            TarCompression::Gz => {
                let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
                let mut builder = tar::Builder::new(enc);
                for source in sources {
                    if cancel_token.is_cancelled() { anyhow::bail!("Operation cancelled"); }
                    add_to_tar_builder(&mut builder, source)?;
                }
                builder.into_inner()?.finish()?;
            }
            TarCompression::None => {
                let mut builder = tar::Builder::new(file);
                for source in sources {
                    if cancel_token.is_cancelled() { anyhow::bail!("Operation cancelled"); }
                    add_to_tar_builder(&mut builder, source)?;
                }
                builder.finish()?;
            }
        }
        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Graceful-degradation stubs for formats without a pure-Rust implementation
// ──────────────────────────────────────────────────────────────────────────────

macro_rules! unsupported_handler {
    ($name:ident, $exts:expr, $extract_msg:literal, $create_msg:literal) => {
        pub struct $name;
        impl $name { pub fn new() -> Self { Self } }
        impl Default for $name { fn default() -> Self { Self::new() } }
        #[async_trait::async_trait]
        impl ArchiveHandler for $name {
            async fn list_entries(&self, _: &Location, _: &CancellationToken) -> Result<Vec<FileEntry>> {
                anyhow::bail!($extract_msg)
            }
            async fn extract_file(&self, _: &Location, _: &Location, _: &CancellationToken) -> Result<()> {
                anyhow::bail!($extract_msg)
            }
            async fn extract_all(&self, _: &Location, _: &Location, _: &CancellationToken) -> Result<()> {
                anyhow::bail!($extract_msg)
            }
            async fn create_archive(&self, _: &[Location], _: &Location, _: &CancellationToken) -> Result<()> {
                anyhow::bail!($create_msg)
            }
            fn is_archive(&self, filename: &str) -> bool {
                let name = filename.to_lowercase();
                $exts.iter().any(|ext| name.ends_with(ext))
            }
        }
    };
}

unsupported_handler!(
    RarArchiveHandler,
    [".rar"],
    "RAR format requires the 'unrar' utility. Install unrar and use it from a shell, or convert the archive to ZIP/7z.",
    "RAR is a proprietary format — rwf cannot create RAR archives. Use ZIP or 7z instead."
);

/// ISO 9660 archive handler (read-only, using the iso9660 crate)
pub struct IsoArchiveHandler;

impl IsoArchiveHandler {
    pub fn new() -> Self { Self }
}

impl Default for IsoArchiveHandler {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl ArchiveHandler for IsoArchiveHandler {
    async fn list_entries(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>> {
        match location {
            Location::Archive { archive_path, inner_path } => {
                match archive_path.as_ref() {
                    Location::Local(path) => {
                        list_iso_entries(path, inner_path, cancel_token)
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
                        extract_iso_single_file(archive_file, inner_path, dest_path, cancel_token)
                    }
                    _ => anyhow::bail!("Only local archives are supported"),
                }
            }
            _ => anyhow::bail!("Invalid source/dest combination for ISO extraction"),
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
                std::fs::create_dir_all(dest_path)?;
                let iso = open_iso(archive_path)?;
                extract_iso_dir_recursive(&iso, "/", dest_path, cancel_token)
            }
            (Location::Archive { archive_path, inner_path }, Location::Local(dest_path)) => {
                match archive_path.as_ref() {
                    Location::Local(archive_file) => {
                        let iso_path = format!("/{}", inner_path.to_string_lossy().replace('\\', "/"));
                        std::fs::create_dir_all(dest_path)?;
                        let iso = open_iso(archive_file)?;
                        extract_iso_dir_recursive(&iso, &iso_path, dest_path, cancel_token)
                    }
                    _ => anyhow::bail!("Only local archives are supported"),
                }
            }
            _ => anyhow::bail!("Invalid archive/dest combination"),
        }
    }

    async fn create_archive(
        &self,
        _sources: &[Location],
        _dest: &Location,
        _cancel_token: &CancellationToken,
    ) -> Result<()> {
        anyhow::bail!("ISO images are read-only disc images — rwf cannot create ISO files.")
    }

    fn is_archive(&self, filename: &str) -> bool {
        filename.to_lowercase().ends_with(".iso")
    }
}

// ── ISO helpers (synchronous, no .await — ISO9660<T> uses Rc internally) ────

fn open_iso(path: &std::path::Path) -> Result<iso9660::ISO9660<std::io::BufReader<std::fs::File>>> {
    let file = std::io::BufReader::new(std::fs::File::open(path)?);
    iso9660::ISO9660::new(file).map_err(|e| anyhow::anyhow!("Failed to open ISO: {}", e))
}

fn list_iso_entries(
    archive_path: &std::path::Path,
    inner_path: &std::path::Path,
    cancel_token: &CancellationToken,
) -> Result<Vec<FileEntry>> {
    use std::time::SystemTime;

    let iso = open_iso(archive_path)?;
    let iso_path = {
        let s = inner_path.to_string_lossy();
        if s.is_empty() { "/".to_string() } else { format!("/{}", s.replace('\\', "/")) }
    };

    let dir_entry = iso.open(&iso_path)
        .map_err(|e| anyhow::anyhow!("ISO navigate error: {}", e))?;
    let dir = match dir_entry {
        Some(iso9660::DirectoryEntry::Directory(d)) => d,
        Some(iso9660::DirectoryEntry::File(_)) => anyhow::bail!("Path is a file, not a directory"),
        None => anyhow::bail!("Directory not found in ISO: {}", iso_path),
    };

    let mut entries = Vec::new();
    for entry in dir.contents() {
        if cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }
        let entry = entry.map_err(|e| anyhow::anyhow!("ISO entry error: {}", e))?;
        let raw_name = entry.identifier();
        // Skip dot/dotdot (identifiers \x00 and \x01)
        if raw_name == "\x00" || raw_name == "\x01" { continue; }
        // ISO 9660 file identifiers have ";1" version suffix — strip it
        let name = raw_name.trim_end_matches(";1").to_string();
        if name.is_empty() { continue; }

        let is_dir = matches!(entry, iso9660::DirectoryEntry::Directory(_));
        let size = match &entry {
            iso9660::DirectoryEntry::File(f) => f.size() as u64,
            iso9660::DirectoryEntry::Directory(_) => 0,
        };
        let entry_inner = if inner_path.components().count() == 0 {
            std::path::PathBuf::from(&name)
        } else {
            inner_path.join(&name)
        };

        entries.push(FileEntry {
            name,
            location: Location::Archive {
                archive_path: Box::new(Location::Local(archive_path.to_path_buf())),
                inner_path: entry_inner,
            },
            size,
            is_dir,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
        });
    }
    Ok(entries)
}

fn extract_iso_single_file(
    archive_path: &std::path::Path,
    inner_path: &std::path::Path,
    dest_path: &std::path::Path,
    _cancel_token: &CancellationToken,
) -> Result<()> {
    let iso = open_iso(archive_path)?;
    let iso_path = format!("/{}", inner_path.to_string_lossy().replace('\\', "/"));
    match iso.open(&iso_path).map_err(|e| anyhow::anyhow!("ISO navigate error: {}", e))? {
        Some(iso9660::DirectoryEntry::File(f)) => {
            if let Some(p) = dest_path.parent() { std::fs::create_dir_all(p)?; }
            let mut reader = f.read();
            let mut out = std::fs::File::create(dest_path)?;
            std::io::copy(&mut reader, &mut out)?;
            Ok(())
        }
        Some(iso9660::DirectoryEntry::Directory(_)) => anyhow::bail!("Path is a directory"),
        None => anyhow::bail!("File not found in ISO: {}", iso_path),
    }
}

fn extract_iso_dir_recursive(
    iso: &iso9660::ISO9660<std::io::BufReader<std::fs::File>>,
    iso_path: &str,
    dest_path: &std::path::Path,
    cancel_token: &CancellationToken,
) -> Result<()> {
    let dir_entry = iso.open(iso_path).map_err(|e| anyhow::anyhow!("ISO navigate error: {}", e))?;
    let dir = match dir_entry {
        Some(iso9660::DirectoryEntry::Directory(d)) => d,
        _ => return Ok(()),
    };

    // Collect names first to avoid borrow issues
    let mut items: Vec<(String, bool)> = Vec::new();
    for entry in dir.contents() {
        let entry = entry.map_err(|e| anyhow::anyhow!("ISO entry error: {}", e))?;
        let raw = entry.identifier();
        if raw == "\x00" || raw == "\x01" { continue; }
        let name = raw.trim_end_matches(";1").to_string();
        if name.is_empty() { continue; }
        let is_dir = matches!(entry, iso9660::DirectoryEntry::Directory(_));
        items.push((name, is_dir));
    }

    for (name, is_dir) in items {
        if cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }
        let out_path = dest_path.join(&name);
        let child_iso_path = if iso_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", iso_path.trim_end_matches('/'), name)
        };
        if is_dir {
            std::fs::create_dir_all(&out_path)?;
            extract_iso_dir_recursive(iso, &child_iso_path, &out_path, cancel_token)?;
        } else if let Some(iso9660::DirectoryEntry::File(f)) =
            iso.open(&child_iso_path).map_err(|e| anyhow::anyhow!("ISO navigate error: {}", e))?
        {
            let mut reader = f.read();
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut reader, &mut out)?;
        }
    }
    Ok(())
}

unsupported_handler!(
    LzhArchiveHandler,
    [".lzh", ".lha"],
    "LZH format is not supported. Convert the archive to ZIP or 7z using an external tool.",
    "LZH is a legacy format — rwf cannot create LZH archives. Use ZIP or 7z instead."
);

// ──────────────────────────────────────────────────────────────────────────────
// MultiFormatArchiveHandler
// ──────────────────────────────────────────────────────────────────────────────

/// Routes archive operations to the correct handler based on file extension.
/// Priority: TAR (.tar/.tgz/.tar.gz) → 7Z (.7z) → ZIP (.zip)
pub struct MultiFormatArchiveHandler {
    zip: ZipArchiveHandler,
    sevenz: SevenZArchiveHandler,
    tar: TarArchiveHandler,
    rar: RarArchiveHandler,
    iso: IsoArchiveHandler,
    lzh: LzhArchiveHandler,
}

impl MultiFormatArchiveHandler {
    pub fn new() -> Self {
        Self {
            zip: ZipArchiveHandler::new(),
            sevenz: SevenZArchiveHandler::new(),
            tar: TarArchiveHandler::new(),
            rar: RarArchiveHandler::new(),
            iso: IsoArchiveHandler::new(),
            lzh: LzhArchiveHandler::new(),
        }
    }

    fn location_name(location: &Location) -> String {
        match location {
            Location::Archive { archive_path, .. } => Self::location_name(archive_path),
            Location::Local(path) => {
                path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase()
            }
            _ => String::new(),
        }
    }

    fn location_is_tar(location: &Location) -> bool {
        let name = Self::location_name(location);
        name.ends_with(".tar") || name.ends_with(".tgz") || name.ends_with(".tar.gz")
    }

    fn location_is_sevenz(location: &Location) -> bool {
        Self::location_name(location).ends_with(".7z")
    }

    fn location_is_rar(location: &Location) -> bool {
        Self::location_name(location).ends_with(".rar")
    }

    fn location_is_iso(location: &Location) -> bool {
        Self::location_name(location).ends_with(".iso")
    }

    fn location_is_lzh(location: &Location) -> bool {
        let name = Self::location_name(location);
        name.ends_with(".lzh") || name.ends_with(".lha")
    }
}

impl Default for MultiFormatArchiveHandler {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl ArchiveHandler for MultiFormatArchiveHandler {
    async fn list_entries(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>> {
        if Self::location_is_tar(location) {
            self.tar.list_entries(location, cancel_token).await
        } else if Self::location_is_sevenz(location) {
            self.sevenz.list_entries(location, cancel_token).await
        } else if Self::location_is_rar(location) {
            self.rar.list_entries(location, cancel_token).await
        } else if Self::location_is_iso(location) {
            self.iso.list_entries(location, cancel_token).await
        } else if Self::location_is_lzh(location) {
            self.lzh.list_entries(location, cancel_token).await
        } else {
            self.zip.list_entries(location, cancel_token).await
        }
    }

    async fn extract_file(
        &self,
        source: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        if Self::location_is_tar(source) {
            self.tar.extract_file(source, dest, cancel_token).await
        } else if Self::location_is_sevenz(source) {
            self.sevenz.extract_file(source, dest, cancel_token).await
        } else if Self::location_is_rar(source) {
            self.rar.extract_file(source, dest, cancel_token).await
        } else if Self::location_is_iso(source) {
            self.iso.extract_file(source, dest, cancel_token).await
        } else if Self::location_is_lzh(source) {
            self.lzh.extract_file(source, dest, cancel_token).await
        } else {
            self.zip.extract_file(source, dest, cancel_token).await
        }
    }

    async fn extract_all(
        &self,
        archive: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        if Self::location_is_tar(archive) {
            self.tar.extract_all(archive, dest, cancel_token).await
        } else if Self::location_is_sevenz(archive) {
            self.sevenz.extract_all(archive, dest, cancel_token).await
        } else if Self::location_is_rar(archive) {
            self.rar.extract_all(archive, dest, cancel_token).await
        } else if Self::location_is_iso(archive) {
            self.iso.extract_all(archive, dest, cancel_token).await
        } else if Self::location_is_lzh(archive) {
            self.lzh.extract_all(archive, dest, cancel_token).await
        } else {
            self.zip.extract_all(archive, dest, cancel_token).await
        }
    }

    async fn create_archive(
        &self,
        sources: &[Location],
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        if Self::location_is_tar(dest) {
            self.tar.create_archive(sources, dest, cancel_token).await
        } else if Self::location_is_sevenz(dest) {
            self.sevenz.create_archive(sources, dest, cancel_token).await
        } else {
            // RAR/ISO/LZH creation is not supported; their handlers return clear errors
            // For dest with those extensions, route to the appropriate stub
            if Self::location_is_rar(dest) {
                self.rar.create_archive(sources, dest, cancel_token).await
            } else if Self::location_is_iso(dest) {
                self.iso.create_archive(sources, dest, cancel_token).await
            } else if Self::location_is_lzh(dest) {
                self.lzh.create_archive(sources, dest, cancel_token).await
            } else {
                self.zip.create_archive(sources, dest, cancel_token).await
            }
        }
    }

    fn is_archive(&self, filename: &str) -> bool {
        self.zip.is_archive(filename)
            || self.sevenz.is_archive(filename)
            || self.tar.is_archive(filename)
            || self.rar.is_archive(filename)
            || self.iso.is_archive(filename)
            || self.lzh.is_archive(filename)
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
