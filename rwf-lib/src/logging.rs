//! Logging configuration and setup
//!
//! This module provides logging functionality with:
//! - Configurable log levels
//! - File logging with rotation at 10MB
//! - Structured logging via tracing

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::Level;
use tracing_subscriber::{
    fmt, fmt::MakeWriter, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

/// Log level configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum LogLevel {
    None,
    Trace,
    Debug,
    #[default]
    Information,
    Warning,
    Error,
    Critical,
}

impl LogLevel {
    /// Convert to tracing Level
    pub fn to_tracing_level(&self) -> Option<Level> {
        match self {
            LogLevel::None => None,
            LogLevel::Trace => Some(Level::TRACE),
            LogLevel::Debug => Some(Level::DEBUG),
            LogLevel::Information => Some(Level::INFO),
            LogLevel::Warning => Some(Level::WARN),
            LogLevel::Error => Some(Level::ERROR),
            LogLevel::Critical => Some(Level::ERROR), // Map Critical to ERROR
        }
    }

    /// Convert to string for filter
    pub fn to_filter_string(&self) -> &'static str {
        match self {
            LogLevel::None => "off",
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Information => "info",
            LogLevel::Warning => "warn",
            LogLevel::Error => "error",
            LogLevel::Critical => "error",
        }
    }
}

/// Rotating file writer that rotates at 10MB
#[derive(Clone)]
pub struct RotatingFileWriter {
    inner: Arc<Mutex<RotatingFileWriterInner>>,
}

struct RotatingFileWriterInner {
    path: PathBuf,
    file: File,
    max_size: u64,
}

impl RotatingFileWriter {
    /// Create a new rotating file writer
    pub fn new(path: PathBuf) -> io::Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new().create(true).append(true).open(&path)?;

        Ok(Self {
            inner: Arc::new(Mutex::new(RotatingFileWriterInner {
                path,
                file,
                max_size: 10 * 1024 * 1024, // 10MB
            })),
        })
    }
}

impl Write for RotatingFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self
            .inner
            .lock()
            .expect("RotatingFileWriter mutex should not be poisoned");

        // Check if rotation is needed
        let needs_rotation = {
            let metadata = inner.file.metadata()?;
            metadata.len() >= inner.max_size
        };

        if needs_rotation {
            // Get path before dropping lock
            let path = inner.path.clone();
            let old_path = path.with_extension("log.old");

            // Release lock before file operations
            drop(inner);

            // Rotate the log file
            if old_path.exists() {
                fs::remove_file(&old_path)?;
            }
            fs::rename(&path, &old_path)?;

            // Reacquire lock and create new file
            let mut inner = self
                .inner
                .lock()
                .expect("RotatingFileWriter mutex should not be poisoned");
            inner.file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&inner.path)?;

            inner.file.write(buf)
        } else {
            inner.file.write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner
            .lock()
            .expect("RotatingFileWriter mutex should not be poisoned")
            .file
            .flush()
    }
}

impl<'a> MakeWriter<'a> for RotatingFileWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Initialize logging with the specified configuration
pub fn init_logging(log_level: LogLevel, log_dir: &Path) -> anyhow::Result<()> {
    if log_level == LogLevel::None {
        return Ok(());
    }

    // Ensure directory exists
    std::fs::create_dir_all(log_dir)?;

    let log_file = log_dir.join("session.log");
    let mut writer = RotatingFileWriter::new(log_file)?;

    // Write session start marker
    let _ = writeln!(
        writer,
        "\n=== Log session started at {} ===",
        chrono::Local::now()
    );

    // Create filter
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level.to_filter_string()));

    // Create file layer
    let file_layer = fmt::layer()
        .with_writer(writer)
        .with_timer(fmt::time::LocalTime::rfc_3339())
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true);

    // Initialize subscriber
    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .init();

    tracing::info!("Logging initialized at level: {:?}", log_level);
    Ok(())
}

/// Get the default log directory
pub fn default_log_dir() -> PathBuf {
    // 1. Check for local logs directory in current working directory
    let local_logs = PathBuf::from("logs");
    if local_logs.exists() && local_logs.is_dir() {
        if let Ok(cwd) = std::env::current_dir() {
            return cwd.join("logs");
        }
    }

    // 2. Fallback to AppData (Roaming on Windows, Config on Unix)
    if let Some(data_dir) = dirs::data_dir() {
        data_dir.join("rwf").join("logs")
    } else {
        PathBuf::from(".").join("logs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::TempDir;

    #[test]
    fn test_rotating_file_writer_basic() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let mut writer = RotatingFileWriter::new(log_path.clone()).unwrap();
        writer.write_all(b"Test log entry\n").unwrap();
        writer.flush().unwrap();

        let mut content = String::new();
        File::open(&log_path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(content.contains("Test log entry"));
    }

    #[test]
    fn test_rotating_file_writer_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let mut writer = RotatingFileWriter::new(log_path.clone()).unwrap();
        // Set small size for testing
        writer.inner.lock().unwrap().max_size = 100;

        // Write enough data to trigger rotation
        for _ in 0..20 {
            writer
                .write_all(b"Test log entry that is long enough\n")
                .unwrap();
        }
        writer.flush().unwrap();

        // Check that old file exists
        let old_path = log_path.with_extension("log.old");
        assert!(old_path.exists());
        assert!(log_path.exists());
    }

    #[test]
    fn test_log_level_conversion() {
        assert_eq!(LogLevel::Trace.to_filter_string(), "trace");
        assert_eq!(LogLevel::Debug.to_filter_string(), "debug");
        assert_eq!(LogLevel::Information.to_filter_string(), "info");
        assert_eq!(LogLevel::Warning.to_filter_string(), "warn");
        assert_eq!(LogLevel::Error.to_filter_string(), "error");
        assert_eq!(LogLevel::None.to_filter_string(), "off");
    }
}
