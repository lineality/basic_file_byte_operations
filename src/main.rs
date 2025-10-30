//! basic_file_byte_operations

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write}, // Seek, SeekFrom,
    path::PathBuf,           // Path,
};

/// Performs an in-place byte replacement operation on a file using a safe copy-and-replace strategy.
///
/// # Overview
/// This function (effectively) "replaces" a single byte at a specified position
/// "in" a file without changing file length. The method is a defensive "build-new-file"
/// approach rather than modifying/changing the original file directly in any way,
/// allowing for a completely unaltered original file in the case of any errors or exceptions.
///
/// # Memory Safety
/// - Uses pre-allocated 64-byte buffer (no heap allocation)
/// - Never loads entire file into memory
/// - Processes file chunk-by-chunk using a "bucket brigade" pattern
/// - No dynamic memory allocation (pre-allocated stack only)
///
/// # File Safety Strategy
/// 1. Creates a backup copy of the original file (.backup extension)
/// 2. Builds a new draft file (.draft extension) with the modified byte
/// 3. Verifies that the operation succeeded
/// 4. Atomically replaces original with draft
/// 5. Removes backup only after verification tests pass and successful completion
///
/// # Operation Behavior
/// - Copies all bytes before target position unchanged
/// - Replaces the byte at target position with new_byte_value
/// - Copies all bytes after target position unchanged
/// - File length remains exactly the same
/// - No frame-shifting occurs
///
/// # Parameters
/// - `original_file_path`: Absolute path to the file to modify
/// - `byte_position_from_start`: Zero-indexed position of byte to replace
/// - `new_byte_value`: The new byte value to write at the specified position
///
/// # Returns
/// - `Ok(())` on successful byte replacement
/// - `Err(io::Error)` if file operations fail or position is invalid
///
/// # Error Conditions
/// - File does not exist
/// - Byte position exceeds file length
/// - Insufficient permissions
/// - Disk full
/// - I/O errors during read/write
///
/// # Recovery Behavior
/// - If operation fails before replacing original, draft is removed, backup remains
/// - If operation fails during replacement, backup file is preserved for manual recovery
/// - Orphaned .draft files indicate incomplete operations
/// - Orphaned .backup files indicate failed replacements
///
/// # Edge Cases
/// - Empty file: Returns error (no bytes to edit)
/// - Position equals file length: Returns error (position out of bounds)
/// - Position > file length: Returns error (position out of bounds)
/// - Single byte file: Replaces that byte if position is 0
/// - Same byte value: Completes operation (idempotent)
/// - Very large files: Processes in chunks, no memory issues
///
/// # Example
/// ```no_run
/// # use std::io;
/// # use std::path::PathBuf;
/// # fn replace_single_byte_in_file(path: PathBuf, pos: usize, byte: u8) -> io::Result<()> { Ok(()) }
/// let file_path = PathBuf::from("/absolute/path/to/file.dat");
/// let position = 1024; // Replace byte at position 1024
/// let new_byte = 0xFF; // Replace with 0xFF
/// let result = replace_single_byte_in_file(file_path, position, new_byte);
/// assert!(result.is_ok());
/// # Ok::<(), io::Error>(())
/// ```
pub fn replace_single_byte_in_file(
    original_file_path: PathBuf,
    byte_position_from_start: usize,
    new_byte_value: u8,
) -> io::Result<()> {
    // =========================================
    // Input Validation Phase
    // =========================================

    println!("=== In-Place Byte Replacement Operation ===");
    println!("Target file: {}", original_file_path.display());
    println!("Byte position: {}", byte_position_from_start);
    println!("New byte value: 0x{:02X}", new_byte_value);
    println!();

    // Verify file exists before any operations
    if !original_file_path.exists() {
        let error_message = format!(
            "Target file does not exist: {}",
            original_file_path.display()
        );
        eprintln!("ERROR: {}", error_message);
        return Err(io::Error::new(io::ErrorKind::NotFound, error_message));
    }

    // Verify file is actually a file, not a directory
    if !original_file_path.is_file() {
        let error_message = format!(
            "Target path is not a file: {}",
            original_file_path.display()
        );
        eprintln!("ERROR: {}", error_message);
        return Err(io::Error::new(io::ErrorKind::InvalidInput, error_message));
    }

    // Get original file metadata for validation
    let original_metadata = fs::metadata(&original_file_path)?;
    let original_file_size = original_metadata.len() as usize;

    // Validate byte position is within file bounds
    if byte_position_from_start >= original_file_size {
        let error_message = format!(
            "Byte position {} exceeds file size {} (valid range: 0-{})",
            byte_position_from_start,
            original_file_size,
            original_file_size.saturating_sub(1)
        );
        eprintln!("ERROR: {}", error_message);
        return Err(io::Error::new(io::ErrorKind::InvalidInput, error_message));
    }

    // Handle empty file case
    if original_file_size == 0 {
        let error_message = "Cannot edit byte in empty file (file size is 0)";
        eprintln!("ERROR: {}", error_message);
        return Err(io::Error::new(io::ErrorKind::InvalidInput, error_message));
    }

    // =========================================
    // Path Construction Phase
    // =========================================

    // Build backup and draft file paths
    let backup_file_path = {
        let mut backup_path = original_file_path.clone();
        let file_name = backup_path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file name"))?
            .to_string_lossy();
        let backup_name = format!("{}.backup", file_name);
        backup_path.set_file_name(backup_name);
        backup_path
    };

    let draft_file_path = {
        let mut draft_path = original_file_path.clone();
        let file_name = draft_path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file name"))?
            .to_string_lossy();
        let draft_name = format!("{}.draft", file_name);
        draft_path.set_file_name(draft_name);
        draft_path
    };

    println!("Backup path: {}", backup_file_path.display());
    println!("Draft path: {}", draft_file_path.display());
    println!();

    // =========================================
    // Backup Creation Phase
    // =========================================

    println!("Creating backup copy...");
    fs::copy(&original_file_path, &backup_file_path).map_err(|e| {
        eprintln!("ERROR: Failed to create backup: {}", e);
        e
    })?;
    println!("Backup created successfully");

    // =========================================
    // Draft File Construction Phase
    // =========================================

    println!("Building modified draft file...");

    // Open original for reading
    let mut source_file = File::open(&original_file_path)?;

    // Create draft file for writing
    let mut draft_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&draft_file_path)?;

    // Pre-allocated buffer for bucket brigade operations
    const BUCKET_BRIGADE_BUFFER_SIZE: usize = 64;
    let mut bucket_brigade_buffer = [0u8; BUCKET_BRIGADE_BUFFER_SIZE];

    // =================================================
    // Debug-Assert, Test-Assert, Production-Catch-Handle
    // =================================================

    // Debug build assertion
    debug_assert!(
        BUCKET_BRIGADE_BUFFER_SIZE > 0,
        "Bucket brigade buffer must have non-zero size"
    );

    // Test build assertion
    #[cfg(test)]
    {
        assert!(
            BUCKET_BRIGADE_BUFFER_SIZE > 0,
            "Bucket brigade buffer must have non-zero size"
        );
    }

    // Production safety check and handle
    if BUCKET_BRIGADE_BUFFER_SIZE == 0 {
        // Clean up draft file on error
        let _ = fs::remove_file(&draft_file_path);
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid buffer configuration",
        ));
    }

    // Tracking variables
    let mut total_bytes_processed: usize = 0;
    let mut chunk_number: usize = 0;
    let mut byte_was_replaced = false;

    // Safety limit to prevent infinite loops
    const MAX_CHUNKS_ALLOWED: usize = 16_777_216; // ~1GB at 64-byte chunks

    // =========================================
    // Main Processing Loop
    // =========================================

    loop {
        // =================================================
        // Debug-Assert, Test-Assert, Production-Catch-Handle
        // =================================================

        // Debug build assertion
        debug_assert!(
            chunk_number < MAX_CHUNKS_ALLOWED,
            "Exceeded maximum chunk limit"
        );

        // Test build assertion
        #[cfg(test)]
        {
            assert!(
                chunk_number < MAX_CHUNKS_ALLOWED,
                "Exceeded maximum chunk limit"
            );
        }

        // Production safety check and handle
        if chunk_number >= MAX_CHUNKS_ALLOWED {
            eprintln!("ERROR: Maximum chunk limit exceeded for safety");
            // Clean up files
            let _ = fs::remove_file(&draft_file_path);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "File too large or infinite loop detected",
            ));
        }

        // Clear buffer before reading (prevent data leakage)
        for i in 0..BUCKET_BRIGADE_BUFFER_SIZE {
            bucket_brigade_buffer[i] = 0;
        }

        chunk_number += 1;

        // Read next chunk from source
        let bytes_read = source_file.read(&mut bucket_brigade_buffer)?;

        // EOF detection
        if bytes_read == 0 {
            println!("Reached end of file");
            break;
        }

        // =================================================
        // Debug-Assert, Test-Assert, Production-Catch-Handle
        // =================================================

        // Debug build assertion
        debug_assert!(
            bytes_read <= BUCKET_BRIGADE_BUFFER_SIZE,
            "Read more bytes than buffer size"
        );

        // Test build assertion
        #[cfg(test)]
        {
            assert!(
                bytes_read <= BUCKET_BRIGADE_BUFFER_SIZE,
                "Read more bytes than buffer size"
            );
        }

        // Production safety check and handle
        if bytes_read > BUCKET_BRIGADE_BUFFER_SIZE {
            eprintln!("ERROR: Buffer overflow detected");
            let _ = fs::remove_file(&draft_file_path);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Buffer overflow in read operation",
            ));
        }

        // Determine if target byte is in this chunk
        let chunk_start_position = total_bytes_processed;
        let chunk_end_position = chunk_start_position + bytes_read;

        // Check if we need to modify a byte in this chunk
        if byte_position_from_start >= chunk_start_position
            && byte_position_from_start < chunk_end_position
        {
            // Calculate position within this chunk
            let position_in_chunk = byte_position_from_start - chunk_start_position;

            // Store original byte for logging
            let original_byte_value = bucket_brigade_buffer[position_in_chunk];

            // Perform the byte replacement
            bucket_brigade_buffer[position_in_chunk] = new_byte_value;
            byte_was_replaced = true;

            println!(
                "Replaced byte at position {}: 0x{:02X} -> 0x{:02X}",
                byte_position_from_start, original_byte_value, new_byte_value
            );
        }

        // Write chunk to draft file
        let bytes_written = draft_file.write(&bucket_brigade_buffer[..bytes_read])?;

        // =================================================
        // Debug-Assert, Test-Assert, Production-Catch-Handle
        // =================================================

        // Debug build assertion
        debug_assert_eq!(bytes_written, bytes_read, "Not all bytes were written");

        // Test build assertion
        #[cfg(test)]
        {
            assert_eq!(bytes_written, bytes_read, "Not all bytes were written");
        }

        // Production safety check and handle
        if bytes_written != bytes_read {
            eprintln!(
                "ERROR: Write mismatch - expected {} bytes, wrote {} bytes",
                bytes_read, bytes_written
            );
            let _ = fs::remove_file(&draft_file_path);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Incomplete write operation",
            ));
        }

        total_bytes_processed += bytes_written;

        // Flush to ensure data is written
        draft_file.flush()?;
    }

    // =========================================
    // Verification Phase
    // =========================================

    println!("\nVerifying operation...");

    // Verify byte was actually replaced
    if !byte_was_replaced {
        eprintln!("ERROR: Target byte position was never reached");
        let _ = fs::remove_file(&draft_file_path);
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Byte replacement did not occur",
        ));
    }

    // Verify file sizes match
    draft_file.flush()?;
    drop(draft_file); // Ensure file is closed
    drop(source_file); // Ensure file is closed

    let draft_metadata = fs::metadata(&draft_file_path)?;
    let draft_size = draft_metadata.len() as usize;

    // =================================================
    // Debug-Assert, Test-Assert, Production-Catch-Handle
    // =================================================

    // Debug build assertion
    debug_assert_eq!(
        draft_size, original_file_size,
        "Draft file size doesn't match original"
    );

    // Test build assertion
    #[cfg(test)]
    {
        assert_eq!(
            draft_size, original_file_size,
            "Draft file size doesn't match original"
        );
    }

    // Production safety check and handle
    if draft_size != original_file_size {
        eprintln!(
            "ERROR: File size mismatch - original: {} bytes, draft: {} bytes",
            original_file_size, draft_size
        );
        let _ = fs::remove_file(&draft_file_path);
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "File size verification failed",
        ));
    }

    println!("File size verified: {} bytes", draft_size);

    // =========================================
    // Atomic Replacement Phase
    // =========================================

    println!("\nReplacing original file with modified version...");

    // Attempt atomic rename (most filesystems support this)
    match fs::rename(&draft_file_path, &original_file_path) {
        Ok(()) => {
            println!("Original file successfully replaced");
        }
        Err(_) => {
            // Fallback: copy and delete (less atomic but more compatible)
            println!("Atomic rename failed, using copy method...");

            fs::copy(&draft_file_path, &original_file_path)?;
            fs::remove_file(&draft_file_path)?;

            println!("Original file successfully replaced (copy method)");
        }
    }

    // =========================================
    // Cleanup Phase
    // =========================================

    println!("\nCleaning up backup file...");

    // Only remove backup after successful replacement
    match fs::remove_file(&backup_file_path) {
        Ok(()) => println!("Backup file removed"),
        Err(e) => {
            // Non-fatal: backup removal failure is not critical
            eprintln!(
                "WARNING: Could not remove backup file: {} ({})",
                backup_file_path.display(),
                e
            );
            println!("Backup file retained at: {}", backup_file_path.display());
        }
    }

    // =========================================
    // Operation Summary
    // =========================================

    println!("\n=== Operation Complete ===");
    println!("File: {}", original_file_path.display());
    println!("Modified position: {}", byte_position_from_start);
    println!("New byte value: 0x{:02X}", new_byte_value);
    println!("Total bytes processed: {}", total_bytes_processed);
    println!("Total chunks: {}", chunk_number);
    println!("Status: SUCCESS");

    Ok(())
}

// =========================================
// Test Module
// =========================================

#[cfg(test)]
mod tests {
    use super::*;
    // use std::io::Write;

    #[test]
    fn test_replace_single_byte_basic() {
        // Create test file
        let test_dir = std::env::temp_dir();
        let test_file = test_dir.join("test_byte_replace.bin");

        // Write test data
        let test_data = vec![0x00, 0x11, 0x22, 0x33, 0x44];
        std::fs::write(&test_file, &test_data).expect("Failed to create test file");

        // Replace byte at position 2 (0x22) with 0xFF
        let result = replace_single_byte_in_file(test_file.clone(), 2, 0xFF);

        assert!(result.is_ok(), "Operation should succeed");

        // Verify result
        let modified_data = std::fs::read(&test_file).expect("Failed to read modified file");
        assert_eq!(modified_data, vec![0x00, 0x11, 0xFF, 0x33, 0x44]);

        // Cleanup
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_replace_byte_position_out_of_bounds() {
        let test_dir = std::env::temp_dir();
        let test_file = test_dir.join("test_byte_bounds.bin");

        // Create small file
        std::fs::write(&test_file, vec![0x00, 0x11]).expect("Failed to create test file");

        // Try to replace byte at invalid position
        let result = replace_single_byte_in_file(
            test_file.clone(),
            10, // Position beyond file size
            0xFF,
        );

        assert!(result.is_err(), "Should fail with out of bounds position");

        // Cleanup
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_replace_byte_empty_file() {
        let test_dir = std::env::temp_dir();
        let test_file = test_dir.join("test_empty.bin");

        // Create empty file
        File::create(&test_file).expect("Failed to create empty file");

        // Try to replace byte in empty file
        let result = replace_single_byte_in_file(test_file.clone(), 0, 0xFF);

        assert!(result.is_err(), "Should fail with empty file");

        // Cleanup
        let _ = std::fs::remove_file(&test_file);
    }
}

/// Three Tests
fn main() -> io::Result<()> {
    // Test 1: Hex-Edit Byte In-Place
    let test_dir_1 = std::env::current_dir()?;
    let original_file_path = test_dir_1.join("pytest_file_1.py");

    let byte_position_from_start: usize = 3;
    let new_byte_value: u8 = 0x61;

    let result_tui =
        replace_single_byte_in_file(original_file_path, byte_position_from_start, new_byte_value);
    println!("result_tui -> {:?}", result_tui);

    // Test 2: Remove Byte

    // Test 3: Add Byte

    println!("main() All Done!");
    Ok(())
}
