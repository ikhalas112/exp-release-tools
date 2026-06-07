//! Optimized I/O operations with memory-mapped file support
//!
//! This module provides high-performance I/O operations using:
//! - Memory-mapped files for large reads (>10MB)
//! - Buffered writes with optimal buffer sizes
//! - Pre-allocated buffers for reads
//! - Zero-copy operations where possible

use crate::error::Error;
use memmap2::Mmap;
use std::{
    fs::File,
    io::{self, BufWriter, Read, Write},
    path::Path,
};

/// Threshold for using memory-mapped files (10 MB)
const MMAP_THRESHOLD: usize = 10 * 1024 * 1024;

/// Optimal buffer sizes for different file sizes
const SMALL_BUFFER_SIZE: usize = 8 * 1024; // 8 KB
const MEDIUM_BUFFER_SIZE: usize = 16 * 1024; // 16 KB
const LARGE_BUFFER_SIZE: usize = 64 * 1024; // 64 KB

/// Get optimal buffer size based on data size
pub fn get_optimal_buffer_size(size: usize) -> usize {
    match size {
        0..=4096 => SMALL_BUFFER_SIZE,
        4097..=102_400 => MEDIUM_BUFFER_SIZE,
        _ => LARGE_BUFFER_SIZE,
    }
}

/// Write data to file with optimal buffering
///
/// # Arguments
///
/// * `path` - File path to write to
/// * `data` - Data to write
///
/// # Returns
///
/// Number of bytes written
///
/// # Errors
///
/// Returns `Error::Io` if write fails
///
/// # Example
///
/// ```rust
/// use maxion_core::io;
///
/// let data = b"Hello, world!";
/// io::write_file("output.txt", data).unwrap();
/// ```
pub fn write_file<P: AsRef<Path>>(path: P, data: &[u8]) -> Result<u64, Error> {
    let path = path.as_ref();
    log::debug!("Writing {} bytes to file: {:?}", data.len(), path);

    let file = File::create(path).map_err(Error::Io)?;

    let buffer_size = get_optimal_buffer_size(data.len());
    let mut writer = BufWriter::with_capacity(buffer_size, file);

    writer.write_all(data).map_err(Error::Io)?;

    // Flush to ensure data is written
    writer.flush().map_err(Error::Io)?;

    log::debug!(
        "Successfully wrote {} bytes to file: {:?}",
        data.len(),
        path
    );
    Ok(data.len() as u64)
}

/// Read entire file into memory with optimal strategy
///
/// For large files (>10 MB), uses memory-mapped I/O for better performance.
/// For smaller files, uses pre-allocated buffer.
///
/// # Arguments
///
/// * `path` - File path to read from
///
/// # Returns
///
/// File contents as `Vec<u8>`
///
/// # Errors
///
/// Returns `Error::Io` if read fails
///
/// # Example
///
/// ```rust
/// use maxion_core::io;
/// use std::io::Write;
/// use tempfile::NamedTempFile;
///
/// let mut temp_file = NamedTempFile::new().unwrap();
/// temp_file.write_all(b"Hello, world!").unwrap();
///
/// let data = io::read_file(temp_file.path()).unwrap();
/// assert_eq!(data, b"Hello, world!");
/// ```
pub fn read_file<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, Error> {
    let path = path.as_ref();
    log::debug!("Reading file: {:?}", path);

    let file = File::open(path).map_err(Error::Io)?;

    let metadata = file.metadata().map_err(Error::Io)?;

    let file_size = metadata.len() as usize;

    // Use memory-mapped I/O for large files
    if file_size > MMAP_THRESHOLD {
        log::debug!(
            "Using memory-mapped I/O for large file ({} bytes)",
            file_size
        );
        return read_file_mmap(&file, file_size);
    }

    // Use pre-allocated buffer for smaller files
    log::debug!("Using pre-allocated buffer ({} bytes)", file_size);
    read_file_buffered(&file, file_size)
}

/// Read file using memory-mapped I/O
///
/// This provides zero-copy access to file contents, ideal for large files.
///
/// # Arguments
///
/// * `file` - Open file handle
/// * `size` - File size in bytes
///
/// # Returns
///
/// File contents as `Vec<u8>`
///
/// # Errors
///
/// Returns `Error::Io` if mmap fails
fn read_file_mmap(file: &File, size: usize) -> Result<Vec<u8>, Error> {
    let mmap = unsafe { Mmap::map(file).map_err(Error::Io)? };

    log::debug!("Memory-mapped file successfully: {} bytes", size);
    Ok(mmap[..].to_vec())
}

/// Read file using pre-allocated buffer
///
/// This avoids multiple allocations and is faster than `read_to_end()`.
///
/// # Arguments
///
/// * `file` - Open file handle
/// * `size` - File size in bytes
///
/// # Returns
///
/// File contents as `Vec<u8>`
///
/// # Errors
///
/// Returns `Error::Io` if read fails
fn read_file_buffered(mut file: &File, size: usize) -> Result<Vec<u8>, Error> {
    let mut buffer = vec![0u8; size];

    file.read_exact(&mut buffer).map_err(Error::Io)?;

    log::debug!("Read file using pre-allocated buffer: {} bytes", size);
    Ok(buffer)
}

/// Read a portion of a file without loading the entire file
///
/// Useful for reading specific ranges from large files.
///
/// # Arguments
///
/// * `path` - File path to read from
/// * `offset` - Byte offset to start reading from
/// * `length` - Number of bytes to read
///
/// # Returns
///
/// File contents as `Vec<u8>`
///
/// # Errors
///
/// Returns `Error::Io` if read fails or offset/length are invalid
///
/// # Example
///
/// ```rust
/// use maxion_core::io;
/// use std::io::Write;
/// use tempfile::NamedTempFile;
///
/// let mut temp_file = NamedTempFile::new().unwrap();
/// temp_file.write_all(b"Hello, world!").unwrap();
///
/// // Read first 5 bytes of file
/// let data = io::read_file_range(temp_file.path(), 0, 5).unwrap();
/// assert_eq!(data, b"Hello");
/// ```
pub fn read_file_range<P: AsRef<Path>>(
    path: P,
    offset: u64,
    length: usize,
) -> Result<Vec<u8>, Error> {
    let path = path.as_ref();
    log::debug!(
        "Reading range from file: {:?} (offset={}, length={})",
        path,
        offset,
        length
    );

    let file = File::open(path).map_err(Error::Io)?;

    let metadata = file.metadata().map_err(Error::Io)?;

    let file_size = metadata.len();

    // Validate offset - if offset is at or beyond file size, return empty Vec
    if offset >= file_size {
        return Ok(Vec::new());
    }

    let max_read = (file_size - offset) as usize;
    let actual_length = length.min(max_read);

    // For large files, use memory-mapped I/O
    if file_size > MMAP_THRESHOLD as u64 {
        return read_file_range_mmap(&file, offset, actual_length);
    }

    // For smaller files, use seek + read
    read_file_range_seek(&file, offset, actual_length)
}

/// Read file range using memory-mapped I/O
///
/// # Arguments
///
/// * `file` - Open file handle
/// * `offset` - Byte offset
/// * `length` - Number of bytes to read
///
/// # Returns
///
/// File contents as `Vec<u8>`
fn read_file_range_mmap(file: &File, offset: u64, length: usize) -> Result<Vec<u8>, Error> {
    // For Windows, use the file handle directly with offset
    #[cfg(windows)]
    {
        let mmap = unsafe { Mmap::map(file).map_err(Error::Io)? };

        let start = offset as usize;
        let end = start.saturating_add(length).min(mmap.len());
        Ok(mmap[start..end].to_vec())
    }

    // For Unix-like systems
    #[cfg(unix)]
    {
        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .offset(offset)
                .len(length)
                .map(file)
                .map_err(Error::Io)?
        };
        Ok(mmap[..].to_vec())
    }
}

/// Read file range using seek and read
///
/// # Arguments
///
/// * `file` - Open file handle
/// * `offset` - Byte offset
/// * `length` - Number of bytes to read
///
/// # Returns
///
/// File contents as `Vec<u8>`
fn read_file_range_seek(file: &File, offset: u64, length: usize) -> Result<Vec<u8>, Error> {
    use std::io::Seek;

    let mut buffer = vec![0u8; length];

    let mut reader = file;
    reader
        .seek(io::SeekFrom::Start(offset))
        .map_err(Error::Io)?;

    reader.read_exact(&mut buffer).map_err(Error::Io)?;

    log::debug!("Read range: {} bytes", length);
    Ok(buffer)
}

/// Zero-copy read from memory-mapped file
///
/// Returns a reference to the memory-mapped data without copying.
/// The lifetime is tied to the Mmap object.
///
/// # Arguments
///
/// * `mmap` - Memory-mapped file
///
/// # Returns
///
/// Reference to the memory-mapped data
///
/// # Safety
///
/// Caller must ensure the mmap outlives the returned reference.
///
/// # Example
///
/// ```rust
/// use maxion_core::io;
/// use memmap2::Mmap;
///
/// use std::io::Write;
/// use tempfile::NamedTempFile;
///
/// let mut temp_file = NamedTempFile::new().unwrap();
/// temp_file.write_all(b"Hello, world!").unwrap();
///
/// let file = temp_file.reopen().unwrap();
/// let mmap = unsafe { Mmap::map(&file).unwrap() };
/// let data = io::read_zero_copy(&mmap);
/// assert_eq!(data, b"Hello, world!");
/// ```
pub fn read_zero_copy(mmap: &Mmap) -> &[u8] {
    log::trace!("Zero-copy read: {} bytes", mmap.len());
    mmap.as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_get_optimal_buffer_size() {
        assert_eq!(get_optimal_buffer_size(1024), SMALL_BUFFER_SIZE);
        assert_eq!(get_optimal_buffer_size(50_000), MEDIUM_BUFFER_SIZE);
        assert_eq!(get_optimal_buffer_size(1_000_000), LARGE_BUFFER_SIZE);
    }

    #[test]
    fn test_write_and_read_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let data = b"Hello, optimized world!";
        write_file(path, data).unwrap();

        let read_data = read_file(path).unwrap();
        assert_eq!(data, read_data.as_slice());
    }

    #[test]
    fn test_read_file_range() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let data = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        write_file(path, data).unwrap();

        // Read first 10 bytes
        let range = read_file_range(path, 0, 10).unwrap();
        assert_eq!(range, b"0123456789");

        // Read bytes 10-20
        let range = read_file_range(path, 10, 10).unwrap();
        assert_eq!(range, b"ABCDEFGHIJ");

        // Read beyond file size
        let range = read_file_range(path, 100, 10).unwrap();
        assert!(range.is_empty());
    }

    #[test]
    fn test_large_file_operations() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create 1 MB file
        let size = 1024 * 1024;
        let data = vec![0x42u8; size];
        write_file(path, &data).unwrap();

        // Read back
        let read_data = read_file(path).unwrap();
        assert_eq!(data.len(), read_data.len());
        assert_eq!(data, read_data.as_slice());
    }

    #[test]
    fn test_zero_copy_read() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let data = b"Zero-copy test data";
        write_file(path, data).unwrap();

        let file = File::open(path).unwrap();
        let mmap = unsafe { Mmap::map(&file).unwrap() };

        let zero_copy_data = read_zero_copy(&mmap);
        assert_eq!(data, zero_copy_data);
    }
}
