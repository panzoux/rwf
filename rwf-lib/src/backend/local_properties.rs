//! Property-based tests for LocalFilesystemBackend
//!
//! **Validates: Requirements 37.6**

use super::local::LocalFilesystemBackend;
use crate::backend::FilesystemBackend;
use crate::model::Location;
use proptest::prelude::*;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

// Strategy for generating directory structures with known sizes
fn directory_structure() -> impl Strategy<Value = (TempDir, Vec<(PathBuf, u64)>, u64)> {
    // Generate 1-10 files with sizes between 100-1000 bytes
    prop::collection::vec(100u64..1000u64, 1..10)
        .prop_map(|file_sizes| {
            let temp_dir = TempDir::new().unwrap();
            let temp_path = temp_dir.path();
            let mut file_paths = Vec::new();
            let mut total_size = 0u64;
            
            // Create files with the generated sizes
            for (i, size) in file_sizes.iter().enumerate() {
                let file_path = temp_path.join(format!("file{}.txt", i));
                let content = vec![0u8; *size as usize];
                std::fs::write(&file_path, content).unwrap();
                file_paths.push((file_path, *size));
                total_size += size;
            }
            
            (temp_dir, file_paths, total_size)
        })
}

// Strategy for generating nested directory structures
fn nested_directory_structure() -> impl Strategy<Value = (TempDir, u64)> {
    // Generate 1-5 subdirectories, each with 1-5 files
    (1..5usize, prop::collection::vec(100u64..500u64, 1..5))
        .prop_map(|(num_subdirs, file_sizes)| {
            let temp_dir = TempDir::new().unwrap();
            let temp_path = temp_dir.path();
            let mut total_size = 0u64;
            
            for subdir_idx in 0..num_subdirs {
                let subdir = temp_path.join(format!("subdir{}", subdir_idx));
                std::fs::create_dir(&subdir).unwrap();
                
                for (file_idx, size) in file_sizes.iter().enumerate() {
                    let file_path = subdir.join(format!("file{}.txt", file_idx));
                    let content = vec![0u8; *size as usize];
                    std::fs::write(&file_path, content).unwrap();
                    total_size += size;
                }
            }
            
            (temp_dir, total_size)
        })
}

proptest! {
    /// **Property 31: Size Calculation Updates Entry**
    ///
    /// For any directory Location, after a CalculateSize job completes successfully,
    /// the corresponding FileEntry should have calculated_size set to Some(size).
    ///
    /// **Validates: Requirements 37.6**
    #[test]
    fn prop_calculate_size_returns_correct_total(
        (temp_dir, _file_paths, expected_size) in directory_structure()
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let backend = LocalFilesystemBackend::new();
            let location = Location::Local(temp_dir.path().to_path_buf());
            let cancel_token = CancellationToken::new();
            
            // Calculate directory size
            let calculated_size = backend
                .calculate_directory_size(&location, &cancel_token)
                .await
                .unwrap();
            
            // The calculated size should match the expected total
            prop_assert_eq!(
                calculated_size,
                expected_size,
                "Calculated size {} should match expected size {}",
                calculated_size,
                expected_size
            );
            
            Ok(())
        })?;
    }

    /// **Property 31: Size Calculation Updates Entry (Nested Directories)**
    ///
    /// For nested directory structures, the size calculation should correctly
    /// sum all files recursively.
    ///
    /// **Validates: Requirements 37.6**
    #[test]
    fn prop_calculate_size_handles_nested_directories(
        (temp_dir, expected_size) in nested_directory_structure()
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let backend = LocalFilesystemBackend::new();
            let location = Location::Local(temp_dir.path().to_path_buf());
            let cancel_token = CancellationToken::new();
            
            // Calculate directory size
            let calculated_size = backend
                .calculate_directory_size(&location, &cancel_token)
                .await
                .unwrap();
            
            // The calculated size should match the expected total
            prop_assert_eq!(
                calculated_size,
                expected_size,
                "Calculated size {} should match expected size {} for nested directories",
                calculated_size,
                expected_size
            );
            
            Ok(())
        })?;
    }

    /// **Property 31: Size Calculation Updates Entry (Empty Directory)**
    ///
    /// For an empty directory, the size calculation should return 0.
    ///
    /// **Validates: Requirements 37.6**
    #[test]
    fn prop_calculate_size_empty_directory_returns_zero(_seed in 0u32..100u32) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let temp_dir = TempDir::new().unwrap();
            let backend = LocalFilesystemBackend::new();
            let location = Location::Local(temp_dir.path().to_path_buf());
            let cancel_token = CancellationToken::new();
            
            // Calculate directory size
            let calculated_size = backend
                .calculate_directory_size(&location, &cancel_token)
                .await
                .unwrap();
            
            // Empty directory should have size 0
            prop_assert_eq!(
                calculated_size,
                0,
                "Empty directory should have size 0, got {}",
                calculated_size
            );
            
            Ok(())
        })?;
    }

    /// **Property 31: Size Calculation Updates Entry (Idempotence)**
    ///
    /// Calculating the size of the same directory multiple times should
    /// return the same result.
    ///
    /// **Validates: Requirements 37.6**
    #[test]
    fn prop_calculate_size_is_idempotent(
        (temp_dir, _file_paths, _expected_size) in directory_structure()
    ) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let backend = LocalFilesystemBackend::new();
            let location = Location::Local(temp_dir.path().to_path_buf());
            let cancel_token = CancellationToken::new();
            
            // Calculate directory size twice
            let size1 = backend
                .calculate_directory_size(&location, &cancel_token)
                .await
                .unwrap();
            
            let size2 = backend
                .calculate_directory_size(&location, &cancel_token)
                .await
                .unwrap();
            
            // Both calculations should return the same result
            prop_assert_eq!(
                size1,
                size2,
                "Size calculation should be idempotent: first={}, second={}",
                size1,
                size2
            );
            
            Ok(())
        })?;
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Test that FileEntry.calculated_size is properly updated after size calculation
    #[tokio::test]
    async fn test_file_entry_calculated_size_updated() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create a directory with known content
        let subdir = temp_path.join("test_dir");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join("file1.txt"), vec![0u8; 100]).unwrap();
        std::fs::write(subdir.join("file2.txt"), vec![0u8; 200]).unwrap();
        
        let backend = LocalFilesystemBackend::new();
        let cancel_token = CancellationToken::new();
        
        // Read the directory to get FileEntry
        let location = Location::Local(temp_path.to_path_buf());
        let mut entries = backend.read_directory(&location, &cancel_token).await.unwrap();
        
        // Find the subdirectory entry
        let dir_entry = entries.iter_mut().find(|e| e.name == "test_dir").unwrap();
        assert!(dir_entry.is_dir);
        assert_eq!(dir_entry.calculated_size, None, "Initially calculated_size should be None");
        
        // Calculate the size
        let calculated_size = backend
            .calculate_directory_size(&dir_entry.location, &cancel_token)
            .await
            .unwrap();
        
        // Update the FileEntry with the calculated size
        dir_entry.calculated_size = Some(calculated_size);
        
        // Verify the calculated_size is now set
        assert_eq!(
            dir_entry.calculated_size,
            Some(300),
            "calculated_size should be set to 300 (100 + 200)"
        );
        
        // Verify formatted_size uses calculated_size
        assert_eq!(dir_entry.formatted_size(), "300 B");
    }

    /// Test that size calculation respects cancellation
    #[tokio::test]
    async fn test_size_calculation_respects_cancellation() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create a large directory structure
        for i in 0..10 {
            let subdir = temp_path.join(format!("subdir{}", i));
            std::fs::create_dir(&subdir).unwrap();
            for j in 0..10 {
                std::fs::write(subdir.join(format!("file{}.txt", j)), vec![0u8; 100]).unwrap();
            }
        }
        
        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(temp_path.to_path_buf());
        let cancel_token = CancellationToken::new();
        
        // Cancel immediately
        cancel_token.cancel();
        
        // The operation should fail with cancellation error
        let result = backend.calculate_directory_size(&location, &cancel_token).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }

    /// Test that size calculation with progress callback works correctly
    #[tokio::test]
    async fn test_size_calculation_with_progress() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create a directory structure with many files to trigger progress reporting
        for i in 0..5 {
            let subdir = temp_path.join(format!("subdir{}", i));
            std::fs::create_dir(&subdir).unwrap();
            for j in 0..25 {
                std::fs::write(subdir.join(format!("file{}.txt", j)), vec![0u8; 100]).unwrap();
            }
        }
        
        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(temp_path.to_path_buf());
        let cancel_token = CancellationToken::new();
        
        // Track progress updates
        let progress_updates = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let progress_updates_clone = progress_updates.clone();
        
        let calculated_size = backend
            .calculate_directory_size_with_progress(&location, &cancel_token, move |items, size| {
                progress_updates_clone.lock().unwrap().push((items, size));
            })
            .await
            .unwrap();
        
        // Verify the total size is correct (5 subdirs * 25 files * 100 bytes = 12500)
        assert_eq!(calculated_size, 12500);
        
        // Verify we received progress updates
        let updates = progress_updates.lock().unwrap();
        assert!(!updates.is_empty(), "Should have received progress updates");
        
        // Verify progress updates are increasing
        for i in 1..updates.len() {
            assert!(updates[i].0 >= updates[i - 1].0, "Items should be increasing");
            assert!(updates[i].1 >= updates[i - 1].1, "Size should be increasing");
        }
    }
}
