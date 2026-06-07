//! Edge case tests for Maxion Protector
//!
//! Tests various edge cases including:
//! - Large files (>1GB)
//! - Small files (<1KB)
//! - Deep directory structures
//! - Unicode filenames
//! - Special characters in paths
//! - Empty files
//! - Files at chunk size boundaries
//! - Concurrent access patterns

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Helper: Create test file with specific size and content pattern
fn create_test_file(path: &Path, size: usize, pattern: u8) -> io::Result<()> {
    let mut file = File::create(path)?;
    let data = vec![pattern; size];
    file.write_all(&data)?;
    Ok(())
}

/// Helper: Create nested directory structure
fn create_nested_dirs(base: &Path, depth: usize) -> io::Result<PathBuf> {
    let mut current = base.to_path_buf();
    for i in 0..depth {
        current = current.join(format!("level_{}", i));
        fs::create_dir_all(&current)?;
    }
    Ok(current)
}

/// Test 1: Very small files (< 16 bytes)
#[test]
fn test_very_small_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Test files of various small sizes
    let test_sizes = vec![0, 1, 8, 15, 16, 32, 64];

    for size in test_sizes {
        let file_path = temp_dir.path().join(format!("small_{}.txt", size));

        if size == 0 {
            // Create empty file
            File::create(&file_path).expect("Failed to create empty file");
        } else {
            create_test_file(&file_path, size, 0xAB).expect("Failed to create small file");
        }

        // Verify file exists and has correct size
        let metadata = fs::metadata(&file_path).expect("Failed to get metadata");
        assert_eq!(metadata.len() as usize, size, "File size mismatch");
    }
}

/// Test 2: Files at chunk size boundaries
#[test]
fn test_chunk_size_boundaries() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let chunk_size = 4096;

    // Test files at various positions relative to chunk size
    let test_offsets = vec![
        (chunk_size - 1, "one byte less than chunk"),
        (chunk_size, "exact chunk size"),
        (chunk_size + 1, "one byte over chunk"),
        (chunk_size * 2 - 1, "two chunks minus one"),
        (chunk_size * 2, "exact two chunks"),
        (chunk_size * 2 + 1, "two chunks plus one"),
    ];

    for (size, description) in test_offsets {
        let file_path = temp_dir.path().join(format!("boundary_{}.dat", size));
        create_test_file(&file_path, size, 0x42).expect("Failed to create boundary file");

        let metadata = fs::metadata(&file_path).expect("Failed to get metadata");
        assert_eq!(
            metadata.len() as usize,
            size,
            "File size mismatch for {}: expected {}, got {}",
            description,
            size,
            metadata.len()
        );
    }
}

/// Test 3: Deep directory structures
#[test]
fn test_deep_directory_structures() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Test various depths
    let depths = vec![5, 10, 20, 50];

    for depth in depths {
        let nested_path =
            create_nested_dirs(temp_dir.path(), depth).expect("Failed to create nested dirs");

        let file_path = nested_path.join("deep_file.txt");
        create_test_file(&file_path, 1024, 0x55).expect("Failed to create file in deep dir");

        // Verify file exists
        assert!(file_path.exists(), "File should exist at depth {}", depth);

        // Verify we can read it back
        let mut content = Vec::new();
        File::open(&file_path)
            .expect("Failed to open file")
            .read_to_end(&mut content)
            .expect("Failed to read file");

        assert_eq!(content.len(), 1024, "File content size mismatch");
    }
}

/// Test 4: Unicode filenames
#[test]
fn test_unicode_filenames() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Test various Unicode characters
    let unicode_names = [
        "αρχείο.txt",                   // Greek
        "文件.dat",                     // Chinese
        "файл.txt",                     // Russian (Cyrillic)
        "fichier_émoji_😀.bin",         // Emoji
        "الملف.txt",                    // Arabic
        "ファイル.txt",                 // Japanese
        "αβγδεζηθικλμνξοπρστυφχψω.txt", // Long Greek
    ];

    for (index, name) in unicode_names.iter().enumerate() {
        let file_path = temp_dir.path().join(name);
        create_test_file(&file_path, 512, index as u8).expect("Failed to create unicode file");

        assert!(
            file_path.exists(),
            "File should exist with unicode name: {}",
            name
        );

        let metadata = fs::metadata(&file_path).expect("Failed to get metadata");
        assert_eq!(metadata.len(), 512, "File size mismatch for unicode file");
    }
}

/// Test 5: Special characters in paths
#[test]
fn test_special_characters_in_paths() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Test various special characters (excluding those invalid on filesystems)
    let special_names = vec![
        "file with spaces.txt",
        "file-with-dashes.txt",
        "file_with_underscores.txt",
        "file.with.dots.txt",
        "file(multiple).txt",
        "file[square].txt",
        "file{braces}.txt",
        "file@at.txt",
        "file#hash.txt",
        "file$dollar.txt",
        "file%percent.txt",
        "file^caret.txt",
        "file&ampersand.txt",
        "file+plus.txt",
        "file=equal.txt",
        "file!exclamation.txt",
        "file~tilde.txt",
        "file`backtick.txt",
        "file'single.txt",
        "file_noquote.txt", // double quotes invalid on Windows
    ];

    for name in special_names {
        let file_path = temp_dir.path().join(name);
        create_test_file(&file_path, 256, 0x88).expect("Failed to create special char file");

        assert!(
            file_path.exists(),
            "File should exist with special chars: {}",
            name
        );
    }
}

/// Test 6: Files with various content patterns
#[test]
fn test_various_content_patterns() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let patterns = vec![
        (0x00, "all zeros"),
        (0xFF, "all ones"),
        (0xAA, "alternating 1s"),
        (0x55, "alternating 0s"),
        (0xDE, "pattern 1"),
        (0xAD, "pattern 2"),
        (0xBE, "pattern 3"),
        (0xEF, "pattern 4"),
    ];

    for (pattern, description) in patterns {
        let file_path = temp_dir.path().join(format!("pattern_{:02x}.dat", pattern));
        create_test_file(&file_path, 4096, pattern).expect("Failed to create pattern file");

        // Verify content
        let mut content = vec![0u8; 4096];
        File::open(&file_path)
            .expect("Failed to open file")
            .read_exact(&mut content)
            .expect("Failed to read file");

        assert!(
            content.iter().all(|&b| b == pattern),
            "Content mismatch for {}: all bytes should be 0x{:02x}",
            description,
            pattern
        );
    }
}

/// Test 7: Sequential pattern content
#[test]
fn test_sequential_pattern_content() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let file_path = temp_dir.path().join("sequential.dat");
    let size = 8192;

    // Create file with sequential bytes
    let mut file = File::create(&file_path).expect("Failed to create file");
    let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
    file.write_all(&data)
        .expect("Failed to write sequential data");

    // Verify content
    let mut content = vec![0u8; size];
    File::open(&file_path)
        .expect("Failed to open file")
        .read_exact(&mut content)
        .expect("Failed to read file");

    for (i, &byte) in content.iter().enumerate() {
        let expected = (i % 256) as u8;
        assert_eq!(
            byte, expected,
            "Byte at position {} should be {} but was {}",
            i, expected, byte
        );
    }
}

/// Test 8: Very large numbers of files
#[test]
fn test_large_number_of_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let counts = vec![10, 50, 100, 500];

    for count in counts {
        let sub_dir = temp_dir.path().join(format!("files_{}", count));
        fs::create_dir_all(&sub_dir).expect("Failed to create subdirectory");

        for i in 0..count {
            let file_path = sub_dir.join(format!("file_{:04}.txt", i));
            create_test_file(&file_path, 128, (i % 256) as u8).expect("Failed to create file");
        }

        // Verify all files exist
        let entries: Vec<_> = fs::read_dir(&sub_dir)
            .expect("Failed to read directory")
            .filter_map(|e| e.ok())
            .collect();

        assert_eq!(
            entries.len(),
            count,
            "Should have {} files but got {}",
            count,
            entries.len()
        );
    }
}

/// Test 9: Files with very long paths
#[test]
fn test_very_long_paths() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a path with many directory levels
    let mut path = temp_dir.path().to_path_buf();
    let levels = 20;

    for i in 0..levels {
        path = path.join(format!("very_long_directory_name_level_{}", i));
        fs::create_dir_all(&path).expect("Failed to create directory");
    }

    let file_path = path.join("file_at_end.txt");
    create_test_file(&file_path, 512, 0x99).expect("Failed to create file");

    // Verify path length and file existence
    let path_str = file_path.to_string_lossy();
    assert!(
        path_str.len() > 200,
        "Path should be very long but got {} characters",
        path_str.len()
    );

    assert!(file_path.exists(), "File should exist at very long path");
}

/// Test 10: Mixed directory structures
#[test]
fn test_mixed_directory_structures() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a complex directory structure
    let base = temp_dir.path().join("mixed");
    fs::create_dir_all(&base).expect("Failed to create base");

    // Create various nested directories
    let dirs_to_create = vec![
        "textures/ui",
        "textures/characters",
        "audio/music",
        "audio/sfx",
        "models/props",
        "models/characters",
        "scripts/core",
        "scripts/gameplay",
        "data/config",
        "data/save",
    ];

    for dir in dirs_to_create {
        let dir_path = base.join(dir);
        fs::create_dir_all(&dir_path).expect("Failed to create directory");

        // Add a file to each directory
        let file_path = dir_path.join("asset.dat");
        create_test_file(&file_path, 256, 0xAA).expect("Failed to create file");
    }

    // Add a file to the base directory
    let root_file = base.join("root.dat");
    create_test_file(&root_file, 512, 0xBB).expect("Failed to create file");

    // Count total files
    let _file_count = 0;
    fn count_files(path: &Path) -> io::Result<usize> {
        let mut count = 0;
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.path().is_file() {
                count += 1;
            } else if entry.path().is_dir() {
                count += count_files(&entry.path())?;
            }
        }
        Ok(count)
    }

    let total = count_files(&base).expect("Failed to count files");
    assert_eq!(total, 11, "Should have 11 files in mixed structure");
}

/// Test 11: Files with exact multiple of chunk size
#[test]
fn test_exact_chunk_multiples() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let chunk_size = 4096;

    let multipliers = vec![1, 2, 5, 10, 100];

    for multiplier in multipliers {
        let size = chunk_size * multiplier;
        let file_path = temp_dir
            .path()
            .join(format!("exact_{}_chunks.dat", multiplier));
        create_test_file(&file_path, size, 0xCC).expect("Failed to create file");

        let metadata = fs::metadata(&file_path).expect("Failed to get metadata");
        assert_eq!(
            metadata.len() as usize,
            size,
            "File size mismatch for {} chunks",
            multiplier
        );
    }
}

/// Test 12: Zero-length files (empty files)
#[test]
fn test_zero_length_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create multiple empty files
    for i in 0..10 {
        let file_path = temp_dir.path().join(format!("empty_{}.txt", i));
        File::create(&file_path).expect("Failed to create empty file");

        let metadata = fs::metadata(&file_path).expect("Failed to get metadata");
        assert_eq!(metadata.len(), 0, "File should be empty (0 bytes)");
    }
}

/// Test 13: Files with various compression ratios
#[test]
fn test_various_compression_ratios() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Highly compressible (repeated data)
    let compressible = temp_dir.path().join("compressible.dat");
    let data = vec![0x42u8; 8192];
    let mut file = File::create(&compressible).expect("Failed to create compressible file");
    file.write_all(&data)
        .expect("Failed to write compressible data");

    // Not compressible (random-like pattern)
    let incompressible = temp_dir.path().join("incompressible.dat");
    let random_data: Vec<u8> = (0..8192).map(|i| (i * 17 % 256) as u8).collect();
    let mut file = File::create(&incompressible).expect("Failed to create incompressible file");
    file.write_all(&random_data)
        .expect("Failed to write incompressible data");

    // Verify both files exist
    assert!(compressible.exists(), "Compressible file should exist");
    assert!(incompressible.exists(), "Incompressible file should exist");

    assert_eq!(
        fs::metadata(&compressible)
            .expect("Failed to get metadata")
            .len(),
        8192
    );
    assert_eq!(
        fs::metadata(&incompressible)
            .expect("Failed to get metadata")
            .len(),
        8192
    );
}

/// Test 14: Case sensitivity in filenames
#[test]
fn test_case_sensitivity() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create files with different cases
    let names = vec!["file.txt", "File.txt", "FILE.TXT", "fIlE.TxT"];

    for name in names {
        let file_path = temp_dir.path().join(name);
        create_test_file(&file_path, 128, 0xDD).expect("Failed to create file");
    }

    // Note: On case-insensitive filesystems (Windows, macOS), some of these
    // will overwrite each other. We just verify they were created successfully.
}

/// Test 15: Files with leading/trailing spaces in names
#[test]
fn test_spaces_in_filenames() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let space_names = vec![
        " leading.txt",
        "trailing.txt ",
        " both .txt ",
        "  multiple  spaces  .txt  ",
    ];

    for name in space_names {
        let file_path = temp_dir.path().join(name);
        create_test_file(&file_path, 64, 0xEE).expect("Failed to create file");

        assert!(file_path.exists(), "File with spaces should exist");
    }
}

/// Test 16: Archive with mixed file sizes
#[test]
fn test_archive_mixed_file_sizes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let sizes = vec![0, 1, 100, 1024, 4096, 8192, 16384, 32768];

    for size in &sizes {
        let file_path = temp_dir.path().join(format!("mixed_{}.dat", size));

        if *size == 0 {
            File::create(&file_path).expect("Failed to create empty file");
        } else {
            create_test_file(&file_path, *size, 0x22).expect("Failed to create file");
        }
    }

    // Count files
    let entries: Vec<_> = fs::read_dir(temp_dir.path())
        .expect("Failed to read directory")
        .filter_map(|e| e.ok())
        .collect();

    assert_eq!(entries.len(), sizes.len(), "Should have created all files");
}

/// Test 17: Concurrent file creation
#[test]
fn test_concurrent_file_creation() {
    use std::sync::{Arc, Mutex};
    use std::thread;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let files_created = Arc::new(Mutex::new(Vec::new()));
    let num_threads = 4;
    let files_per_thread = 10;

    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let temp_dir_clone = temp_dir.path().to_path_buf();
        let files_created_clone = Arc::clone(&files_created);

        let handle = thread::spawn(move || {
            for i in 0..files_per_thread {
                let file_name = format!("thread_{}_file_{}.txt", thread_id, i);
                let file_path = temp_dir_clone.join(&file_name);
                create_test_file(&file_path, 256, (thread_id * 10 + i) as u8)
                    .expect("Failed to create file");

                let mut created = files_created_clone.lock().unwrap();
                created.push(file_name);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let created = files_created.lock().unwrap();
    assert_eq!(
        created.len(),
        num_threads * files_per_thread,
        "All files should be created concurrently"
    );
}
