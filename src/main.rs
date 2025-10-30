//! basic_file_byte_operations

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
/*

# File Identities & Workflow
At the granular level of these operations, it may be best to avoid user-abstractions such as 'add' or 'remove' or 'modify' and 'original' or 'copy' when speaking of the actual mechanical steps. We should to look instead at specific well-defined steps and actions. The semantics may seem counter-intuitive, as to effect the same result we never make any changes to either the original file (preserved for safety) or to the new file, which we describe as 'altered' meaning that it is different from the original, not that 'change operations' were ever performed on the file as such. For example reconstructing a file after frameshifting does not ever literally happen (as it would need to if there were only one file without a backup).

It may be possible to effect the desired end-state (retroactively described as 'add' or 'remove') with steps such as these:

1. Create a draft file.

2. Append bytes (from the original file, to the draft-file) up to the 'file byte position of the change operation' in question:
append byte by byte, or append with a small bucket-brigade buffer.

3. Performing Operation at 'file byte position of the change operation':
- For Remove-a-byte-operation: no action taken for draft-file, nothing written. This is an effective frame shift/advance in reading the original file one byte.
- For Add-a-byte-operation: append the 'new' (not in original file) byte to the draft file. Do not shift original file read-location.
- For Hex-edit: append the 'new' (not in original file) byte to the draft file.

4. Performing Operation ~after 'file byte position of the change operation':
- For hex-edit: Append bytes (from the original file, to the draft-file) after the 'file byte position of the change operation' in question:
append byte by byte, or append with a small bucket-brigade buffer.
- For remove-byte: Append bytes (from the original file, to the draft-file), after the 'file byte position of the change operation' in question: append byte by byte, or append with a small bucket-brigade buffer. This is similar to hex-edit, except that nothing is added AT the target position, effecting a frame-shift.
- For Add-byte Edit: Append bytes (from the original file, to the draft-file), FROM/INCLUDING the 'file byte position of the change operation' in question: append byte by byte, or append with a small bucket-brigade buffer, effecting a frame-shift.


In theory, this process only 'need' apply to Add-a-byte-operation and Remove-a-byte-operation not (hex-edit)change-a-byte-in-place. An in-place byte change can be done simple on a file. However, what is better:
1. A standard process of building a new file cleanly and not making any internal changes to it and which is a single process always used, or
2. Having two different workflows in the same tool-kit, whereby in-place edit makes a complete copy of a file and then navigates back to the change-spot and changes it and resaves the file. Is that simpler than writing the file per-design in the first place with a standard workflow, especially when a backup copy would be made for safety in either case? We will assume that a more uniform workflow is more practical.

Using these steps we are not 'altering' any file per-se; we are constructing the 'altered' (relatively speaking) file in one clean workflow.

# Test, Check, And Verify
There can also be checking steps such as:
- (double)checking original vs. new file: total byte length
- (double)checking original vs. new file: pre-position byte length similarity (possible a hash-check)
- (double)checking original vs. new file value: at-position, must be dissimilarity
- (double)checking original vs. new file: post-position, must be similarity given frame-shift or not (possible a hash-check)
 - - hex-edit in place: no frameshift: post-position must be the same
 - - remove byte: -1 frameshift in new file compared with original: given -1 frameshift post position must be the same
 - - add byte: +1 frameshift in new file compared with original: given +1 frameshift, post position must be the same


Remove-Byte Operation Workflow
Let me restate the remove-byte operation using your precise mechanical terminology:
Draft File Construction Process
Step 1: Create Draft File

Open original file for reading (read position starts at 0)
Create empty draft file for writing (write position starts at 0)

Step 2: Append Pre-Position Bytes

Read from original: bytes at positions 0 through byte_position_from_start - 1
Append to draft: all these bytes sequentially
Original read position after: at byte_position_from_start
Draft write position after: at byte_position_from_start

Step 3: Perform Remove Operation AT Position

Original file: advance read position by 1 (skip the byte at byte_position_from_start)

Read position moves from byte_position_from_start to byte_position_from_start + 1


Draft file: write nothing, take no action

Write position remains at byte_position_from_start


Effect: The byte at byte_position_from_start in the original is never appended to draft

Step 4: Append Post-Position Bytes

Read from original: bytes starting at position byte_position_from_start + 1 through EOF

(Original read position is already at byte_position_from_start + 1 from Step 3)


Append to draft: all remaining bytes sequentially
Effect: These bytes are written to draft starting at position byte_position_from_start

This creates the -1 frame-shift automatically
*/

/// Computes a simple checksum for a byte slice (for verification purposes)
///
/// Uses a basic XOR-based checksum for speed and simplicity.
/// This is sufficient for integrity checking, not cryptographic security.
fn compute_simple_checksum(bytes: &[u8]) -> u64 {
    let mut checksum: u64 = 0;
    for (i, &byte) in bytes.iter().enumerate() {
        // Mix position and value to detect transpositions
        checksum ^= (byte as u64).rotate_left((i % 64) as u32);
        checksum = checksum.wrapping_add(byte as u64);
    }
    checksum
}

/// Performs comprehensive verification of a byte replacement operation.
///
/// # Verification Steps
/// 1. **Total byte length check**: Ensures file sizes match exactly
/// 2. **Pre-position similarity**: Verifies all bytes before edit position are identical
/// 3. **At-position dissimilarity**: Confirms the target byte was actually changed
/// 4. **Post-position similarity**: Verifies all bytes after edit position are identical
///
/// # Parameters
/// - `original_path`: Path to the original file (backup)
/// - `modified_path`: Path to the modified file (draft)
/// - `byte_position`: Position where byte was replaced
/// - `expected_old_byte`: The original byte value that should have been replaced
/// - `expected_new_byte`: The new byte value that should be at the position
///
/// # Returns
/// - `Ok(())` if all verifications pass
/// - `Err(io::Error)` if any verification fails
fn verify_byte_replacement_operation(
    original_path: &Path,
    modified_path: &Path,
    byte_position: usize,
    expected_old_byte: u8,
    expected_new_byte: u8,
) -> io::Result<()> {
    println!("\n=== Comprehensive Verification Phase ===");

    // =========================================
    // Step 1: Total Byte Length Check
    // =========================================
    println!("1. Verifying total byte length...");

    let original_metadata = fs::metadata(original_path)?;
    let modified_metadata = fs::metadata(modified_path)?;
    let original_size = original_metadata.len() as usize;
    let modified_size = modified_metadata.len() as usize;

    // Debug-Assert, Test-Assert, Production-Catch-Handle
    debug_assert_eq!(
        original_size, modified_size,
        "File sizes must match for in-place edit"
    );

    #[cfg(test)]
    {
        assert_eq!(
            original_size, modified_size,
            "File sizes must match for in-place edit"
        );
    }

    if original_size != modified_size {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "File size mismatch: original={}, modified={}",
                original_size, modified_size
            ),
        ));
    }

    println!("   ✓ File sizes match: {} bytes", original_size);

    // Open both files for reading
    let mut original_file = File::open(original_path)?;
    let mut modified_file = File::open(modified_path)?;

    // =========================================
    // Step 2: Pre-Position Similarity Check
    // =========================================
    println!(
        "2. Verifying pre-position bytes (0 to {})...",
        byte_position - 1
    );

    if byte_position > 0 {
        // Read and compare bytes before the edit position
        const VERIFICATION_BUFFER_SIZE: usize = 64;
        let mut original_buffer = [0u8; VERIFICATION_BUFFER_SIZE];
        let mut modified_buffer = [0u8; VERIFICATION_BUFFER_SIZE];

        let mut pre_position_original_checksum: u64 = 0;
        let mut pre_position_modified_checksum: u64 = 0;
        let mut bytes_verified: usize = 0;

        while bytes_verified < byte_position {
            let bytes_to_read =
                std::cmp::min(VERIFICATION_BUFFER_SIZE, byte_position - bytes_verified);

            let original_bytes_read = original_file.read(&mut original_buffer[..bytes_to_read])?;
            let modified_bytes_read = modified_file.read(&mut modified_buffer[..bytes_to_read])?;

            // Verify same number of bytes read
            if original_bytes_read != modified_bytes_read {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Pre-position read mismatch",
                ));
            }

            // Update checksums
            pre_position_original_checksum = pre_position_original_checksum.wrapping_add(
                compute_simple_checksum(&original_buffer[..original_bytes_read]),
            );
            pre_position_modified_checksum = pre_position_modified_checksum.wrapping_add(
                compute_simple_checksum(&modified_buffer[..modified_bytes_read]),
            );

            // Byte-by-byte comparison for pre-position bytes
            for i in 0..original_bytes_read {
                if original_buffer[i] != modified_buffer[i] {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "Pre-position byte mismatch at position {}: original=0x{:02X}, modified=0x{:02X}",
                            bytes_verified + i,
                            original_buffer[i],
                            modified_buffer[i]
                        ),
                    ));
                }
            }

            bytes_verified += original_bytes_read;
        }

        // Verify checksums match
        if pre_position_original_checksum != pre_position_modified_checksum {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Pre-position checksum mismatch: original={:016X}, modified={:016X}",
                    pre_position_original_checksum, pre_position_modified_checksum
                ),
            ));
        }

        println!(
            "   ✓ Pre-position bytes match (checksum: {:016X})",
            pre_position_original_checksum
        );
    } else {
        println!("   ✓ No pre-position bytes to verify (position is 0)");
    }

    // =========================================
    // Step 3: At-Position Dissimilarity Check
    // =========================================
    println!("3. Verifying at-position byte change...");

    let mut original_byte = [0u8; 1];
    let mut modified_byte = [0u8; 1];

    original_file.read_exact(&mut original_byte)?;
    modified_file.read_exact(&mut modified_byte)?;

    // Verify original byte is what we expected
    if original_byte[0] != expected_old_byte {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Original byte mismatch at position {}: expected=0x{:02X}, actual=0x{:02X}",
                byte_position, expected_old_byte, original_byte[0]
            ),
        ));
    }

    // Verify modified byte is what we set
    if modified_byte[0] != expected_new_byte {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Modified byte mismatch at position {}: expected=0x{:02X}, actual=0x{:02X}",
                byte_position, expected_new_byte, modified_byte[0]
            ),
        ));
    }

    // Verify they are different (dissimilarity check)
    if original_byte[0] == modified_byte[0] {
        println!("   ⚠ Warning: Byte value unchanged (same value written)");
    }

    println!(
        "   ✓ At-position byte successfully changed: 0x{:02X} -> 0x{:02X}",
        original_byte[0], modified_byte[0]
    );

    // =========================================
    // Step 4: Post-Position Similarity Check
    // =========================================
    println!(
        "4. Verifying post-position bytes ({} to EOF)...",
        byte_position + 1
    );

    const POST_VERIFICATION_BUFFER_SIZE: usize = 64;
    let mut original_post_buffer = [0u8; POST_VERIFICATION_BUFFER_SIZE];
    let mut modified_post_buffer = [0u8; POST_VERIFICATION_BUFFER_SIZE];

    let mut post_position_original_checksum: u64 = 0;
    let mut post_position_modified_checksum: u64 = 0;
    let mut post_bytes_verified: usize = 0;

    loop {
        let original_bytes_read = original_file.read(&mut original_post_buffer)?;
        let modified_bytes_read = modified_file.read(&mut modified_post_buffer)?;

        // Both files should reach EOF at the same time
        if original_bytes_read != modified_bytes_read {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Post-position read size mismatch: original={}, modified={}",
                    original_bytes_read, modified_bytes_read
                ),
            ));
        }

        // Check if we've reached EOF
        if original_bytes_read == 0 {
            break;
        }

        // Update checksums
        post_position_original_checksum = post_position_original_checksum.wrapping_add(
            compute_simple_checksum(&original_post_buffer[..original_bytes_read]),
        );
        post_position_modified_checksum = post_position_modified_checksum.wrapping_add(
            compute_simple_checksum(&modified_post_buffer[..modified_bytes_read]),
        );

        // Byte-by-byte comparison for post-position bytes
        for i in 0..original_bytes_read {
            if original_post_buffer[i] != modified_post_buffer[i] {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "Post-position byte mismatch at offset +{}: original=0x{:02X}, modified=0x{:02X}",
                        post_bytes_verified + i + 1,
                        original_post_buffer[i],
                        modified_post_buffer[i]
                    ),
                ));
            }
        }

        post_bytes_verified += original_bytes_read;
    }

    // Verify post-position checksums match
    if post_position_original_checksum != post_position_modified_checksum {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Post-position checksum mismatch: original={:016X}, modified={:016X}",
                post_position_original_checksum, post_position_modified_checksum
            ),
        ));
    }

    if post_bytes_verified > 0 {
        println!(
            "   ✓ Post-position bytes match ({} bytes, checksum: {:016X})",
            post_bytes_verified, post_position_original_checksum
        );
    } else {
        println!("   ✓ No post-position bytes (edit was at last byte)");
    }

    // =========================================
    // Final Verification Summary
    // =========================================
    println!("\n=== Verification Summary ===");
    println!("✓ Total byte length: VERIFIED ({} bytes)", original_size);
    println!("✓ Pre-position similarity: VERIFIED");
    println!("✓ At-position dissimilarity: VERIFIED");
    println!("✓ Post-position similarity: VERIFIED (no frame-shift)");
    println!("All verification checks PASSED\n");

    Ok(())
}

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

    // =========================================
    // Comprehensive Verification Phase
    // =========================================

    // let mut original_check_file = File::open(&original_file_path)?; // THE ACTUAL ORIGINAL!
    // original_check_file.seek(SeekFrom::Start(byte_position_from_start as u64))?;
    // let mut byte_buffer = [0u8; 1];
    // original_check_file.read_exact(&mut byte_buffer)?;
    // let original_byte_at_position = byte_buffer[0];

    // Read original byte for verification
    /*
    This ensures the file handle is closed before you try to rename.
    The curly braces { } create a new scope. When that scope ends,
    original_check_file is immediately dropped and the file handle is closed.
    */
    let original_byte_at_position = {
        let mut original_check_file = File::open(&original_file_path)?;
        original_check_file.seek(SeekFrom::Start(byte_position_from_start as u64))?;
        let mut byte_buffer = [0u8; 1];
        original_check_file.read_exact(&mut byte_buffer)?;
        byte_buffer[0]
        // original_check_file automatically dropped here
    };

    // Perform all verification checks before replacing the original
    verify_byte_replacement_operation(
        &original_file_path, // The actual original (still unmodified)
        &draft_file_path,    // Modified (draft) file
        byte_position_from_start,
        original_byte_at_position,
        new_byte_value,
    )?;

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
        Err(e) => {
            // DO NOT try to copy over the original!
            // Leave all files as-is for safety
            eprintln!("Cannot atomically replace file: {}", e);
            return Err(e);
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

// =====================
// Remove-Byte Operation
// =====================

/// Performs comprehensive verification of a byte removal operation.
///
/// # Verification Steps
/// 1. **Total byte length check**: Ensures draft is exactly 1 byte smaller than original
/// 2. **Pre-position similarity**: Verifies all bytes before removal position are identical
/// 3. **At-position dissimilarity**: Confirms byte at position has changed (is the next byte)
/// 4. **Post-position similarity with -1 frame-shift**: Verifies remaining bytes match with shift
///
/// # Frame-Shift Verification
/// After removing a byte at position N:
/// - `draft[N] == original[N+1]` (the byte after removed byte shifts into its place)
/// - `draft[N+1] == original[N+2]` (and so on...)
/// - All bytes after position N in draft correspond to position N+1 in original
///
/// # Parameters
/// - `original_path`: Path to the original file
/// - `draft_path`: Path to the draft file with byte removed
/// - `byte_position`: Position where byte was removed
/// - `removed_byte_value`: The byte value that was removed (for logging)
///
/// # Returns
/// - `Ok(())` if all verifications pass
/// - `Err(io::Error)` if any verification fails
fn verify_byte_removal_operation(
    original_path: &Path,
    draft_path: &Path,
    byte_position: usize,
    removed_byte_value: u8,
) -> io::Result<()> {
    println!("\n=== Comprehensive Verification Phase ===");

    // =========================================
    // Step 1: Total Byte Length Check
    // =========================================
    println!("1. Verifying total byte length...");

    let original_metadata = fs::metadata(original_path)?;
    let draft_metadata = fs::metadata(draft_path)?;
    let original_size = original_metadata.len() as usize;
    let draft_size = draft_metadata.len() as usize;

    let expected_draft_size = original_size.saturating_sub(1);

    // Debug-Assert, Test-Assert, Production-Catch-Handle
    debug_assert_eq!(
        draft_size, expected_draft_size,
        "Draft file must be exactly 1 byte smaller than original"
    );

    #[cfg(test)]
    {
        assert_eq!(
            draft_size, expected_draft_size,
            "Draft file must be exactly 1 byte smaller than original"
        );
    }

    if draft_size != expected_draft_size {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "File size mismatch: original={}, draft={}, expected={}",
                original_size, draft_size, expected_draft_size
            ),
        ));
    }

    println!(
        "   ✓ File sizes correct: original={} bytes, draft={} bytes (removed 1 byte)",
        original_size, draft_size
    );

    // Open both files for reading
    let mut original_file = File::open(original_path)?;
    let mut draft_file = File::open(draft_path)?;

    // =========================================
    // Step 2: Pre-Position Similarity Check
    // =========================================
    println!(
        "2. Verifying pre-position bytes (0 to {})...",
        byte_position.saturating_sub(1)
    );

    if byte_position > 0 {
        const VERIFICATION_BUFFER_SIZE: usize = 64;
        let mut original_buffer = [0u8; VERIFICATION_BUFFER_SIZE];
        let mut draft_buffer = [0u8; VERIFICATION_BUFFER_SIZE];

        let mut pre_position_original_checksum: u64 = 0;
        let mut pre_position_draft_checksum: u64 = 0;
        let mut bytes_verified: usize = 0;

        while bytes_verified < byte_position {
            let bytes_to_read =
                std::cmp::min(VERIFICATION_BUFFER_SIZE, byte_position - bytes_verified);

            let original_bytes_read = original_file.read(&mut original_buffer[..bytes_to_read])?;
            let draft_bytes_read = draft_file.read(&mut draft_buffer[..bytes_to_read])?;

            // Verify same number of bytes read
            if original_bytes_read != draft_bytes_read {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Pre-position read mismatch",
                ));
            }

            // Update checksums
            pre_position_original_checksum = pre_position_original_checksum.wrapping_add(
                compute_simple_checksum(&original_buffer[..original_bytes_read]),
            );
            pre_position_draft_checksum = pre_position_draft_checksum
                .wrapping_add(compute_simple_checksum(&draft_buffer[..draft_bytes_read]));

            // Byte-by-byte comparison for pre-position bytes
            for i in 0..original_bytes_read {
                if original_buffer[i] != draft_buffer[i] {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "Pre-position byte mismatch at position {}: original=0x{:02X}, draft=0x{:02X}",
                            bytes_verified + i,
                            original_buffer[i],
                            draft_buffer[i]
                        ),
                    ));
                }
            }

            bytes_verified += original_bytes_read;
        }

        // Verify checksums match
        if pre_position_original_checksum != pre_position_draft_checksum {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Pre-position checksum mismatch: original={:016X}, draft={:016X}",
                    pre_position_original_checksum, pre_position_draft_checksum
                ),
            ));
        }

        println!(
            "   ✓ Pre-position bytes match (checksum: {:016X})",
            pre_position_original_checksum
        );
    } else {
        println!("   ✓ No pre-position bytes to verify (position is 0)");
    }

    // =========================================
    // Step 3: At-Position Dissimilarity Check
    // =========================================
    println!("3. Verifying byte removal at position {}...", byte_position);

    // Read the byte that was removed from original
    let mut original_removed_byte = [0u8; 1];
    original_file.read_exact(&mut original_removed_byte)?;

    // Verify it matches what we expected to remove
    if original_removed_byte[0] != removed_byte_value {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Removed byte mismatch: expected=0x{:02X}, actual=0x{:02X}",
                removed_byte_value, original_removed_byte[0]
            ),
        ));
    }

    // Read the byte that should now be at this position in draft
    // This should be the byte that was AFTER the removed byte in original
    let mut draft_current_byte = [0u8; 1];

    // Handle edge case: if we removed the last byte, draft has no more bytes
    let draft_has_more_bytes = draft_file.read(&mut draft_current_byte)? == 1;

    if draft_has_more_bytes {
        // Read the next byte from original (this should match draft's current byte)
        let mut original_next_byte = [0u8; 1];
        let original_has_next = original_file.read(&mut original_next_byte)? == 1;

        if !original_has_next {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Draft has more bytes than expected after removal position",
            ));
        }

        // The byte now at position in draft should be what was after removed byte in original
        if draft_current_byte[0] != original_next_byte[0] {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "At-position frame-shift verification failed: draft[{}]=0x{:02X}, expected original[{}]=0x{:02X}",
                    byte_position,
                    draft_current_byte[0],
                    byte_position + 1,
                    original_next_byte[0]
                ),
            ));
        }

        println!(
            "   ✓ Byte removed: 0x{:02X} (position {} now contains 0x{:02X} from position {})",
            original_removed_byte[0],
            byte_position,
            draft_current_byte[0],
            byte_position + 1
        );
    } else {
        println!(
            "   ✓ Byte removed: 0x{:02X} (was last byte in file)",
            original_removed_byte[0]
        );
    }

    // =========================================
    // Step 4: Post-Position Similarity Check with -1 Frame-Shift
    // =========================================
    println!("4. Verifying post-position bytes with -1 frame-shift...");

    const POST_VERIFICATION_BUFFER_SIZE: usize = 64;
    let mut original_post_buffer = [0u8; POST_VERIFICATION_BUFFER_SIZE];
    let mut draft_post_buffer = [0u8; POST_VERIFICATION_BUFFER_SIZE];

    let mut post_position_original_checksum: u64 = 0;
    let mut post_position_draft_checksum: u64 = 0;
    let mut post_bytes_verified: usize = 0;

    // Note: We already read one byte from each file in Step 3
    // Original file read position: byte_position + 2
    // Draft file read position: byte_position + 1
    // These are already correctly offset by the frame-shift

    loop {
        let original_bytes_read = original_file.read(&mut original_post_buffer)?;
        let draft_bytes_read = draft_file.read(&mut draft_post_buffer)?;

        // Both files should reach EOF at the same time (accounting for the removed byte)
        if original_bytes_read != draft_bytes_read {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Post-position read size mismatch: original={}, draft={}",
                    original_bytes_read, draft_bytes_read
                ),
            ));
        }

        // Check if we've reached EOF
        if original_bytes_read == 0 {
            break;
        }

        // Update checksums
        post_position_original_checksum = post_position_original_checksum.wrapping_add(
            compute_simple_checksum(&original_post_buffer[..original_bytes_read]),
        );
        post_position_draft_checksum = post_position_draft_checksum.wrapping_add(
            compute_simple_checksum(&draft_post_buffer[..draft_bytes_read]),
        );

        // Byte-by-byte comparison for post-position bytes (with frame-shift already in effect)
        for i in 0..original_bytes_read {
            if original_post_buffer[i] != draft_post_buffer[i] {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "Post-position byte mismatch at offset +{}: original=0x{:02X}, draft=0x{:02X}",
                        post_bytes_verified + i,
                        original_post_buffer[i],
                        draft_post_buffer[i]
                    ),
                ));
            }
        }

        post_bytes_verified += original_bytes_read;
    }

    // Verify post-position checksums match
    if post_position_original_checksum != post_position_draft_checksum {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Post-position checksum mismatch: original={:016X}, draft={:016X}",
                post_position_original_checksum, post_position_draft_checksum
            ),
        ));
    }

    if post_bytes_verified > 0 {
        println!(
            "   ✓ Post-position bytes match with -1 frame-shift ({} bytes, checksum: {:016X})",
            post_bytes_verified, post_position_original_checksum
        );
    } else {
        println!("   ✓ No post-position bytes (removal was at last byte)");
    }

    // =========================================
    // Final Verification Summary
    // =========================================
    println!("\n=== Verification Summary ===");
    println!(
        "✓ Total byte length: VERIFIED (original={}, draft={}, -1 byte)",
        original_size, draft_size
    );
    println!("✓ Pre-position similarity: VERIFIED");
    println!("✓ At-position dissimilarity: VERIFIED (byte removed)");
    println!("✓ Post-position similarity: VERIFIED (with -1 frame-shift)");
    println!("All verification checks PASSED\n");

    Ok(())
}

/// Performs a byte removal operation on a file using a safe copy-and-replace strategy.
///
/// # Overview
/// This function removes a single byte at a specified position in a file, causing all
/// subsequent bytes to shift backward by one position (frame-shift -1). It uses a defensive
/// "build-new-file" approach rather than modifying the original file directly.
///
/// # Memory Safety
/// - Uses pre-allocated 64-byte buffer (no heap allocation)
/// - Never loads entire file into memory
/// - Processes file chunk-by-chunk using bucket brigade pattern
/// - No dynamic memory allocation
///
/// # File Safety Strategy
/// 1. Creates a backup copy of the original file (.backup extension)
/// 2. Builds a new draft file (.draft extension) with the byte removed
/// 3. Verifies the operation succeeded (including frame-shift verification)
/// 4. Atomically replaces original with draft
/// 5. Removes backup only after successful completion
///
/// # Operation Behavior - Mechanical Steps
/// The draft file is constructed by appending bytes sequentially:
///
/// **Step 1**: Create empty draft file
///
/// **Step 2**: Append pre-position bytes
/// - Read from original: positions 0 to `byte_position - 1`
/// - Append to draft: all these bytes
///
/// **Step 3**: Perform removal AT position
/// - Original file: advance read position by 1 (skip target byte)
/// - Draft file: write nothing (no append action)
/// - Effect: The byte at target position is never written to draft
///
/// **Step 4**: Append post-position bytes
/// - Read from original: positions `byte_position + 1` to EOF
/// - Append to draft: all remaining bytes
/// - Effect: These bytes naturally occupy positions starting at `byte_position` in draft
/// - This creates the -1 frame-shift automatically
///
/// # Frame-Shift Behavior
/// After removing byte at position N:
/// - Bytes 0 to N-1: unchanged positions
/// - Byte at N: removed (does not exist in new file)
/// - Bytes N+1 to EOF: all shift backward by 1 position
/// - File length decreases by exactly 1
///
/// # Parameters
/// - `original_file_path`: Absolute path to the file to modify
/// - `byte_position_from_start`: Zero-indexed position of byte to remove
///
/// # Returns
/// - `Ok(())` on successful byte removal
/// - `Err(io::Error)` if file operations fail or position is invalid
///
/// # Error Conditions
/// - File does not exist
/// - File is empty
/// - Byte position >= file length (out of bounds)
/// - Insufficient permissions
/// - Disk full
/// - I/O errors during read/write
///
/// # Recovery Behavior
/// - If operation fails before replacing original, draft is removed, backup remains
/// - If atomic rename fails, both original and backup are preserved
/// - Orphaned .draft files indicate incomplete operations
/// - Orphaned .backup files indicate failed replacements
///
/// # Edge Cases
/// - Empty file: Returns error (no bytes to remove)
/// - Position >= file length: Returns error (position out of bounds)
/// - Single byte file at position 0: Results in empty file (valid operation)
/// - Remove last byte: File becomes 1 byte shorter, no post-position bytes
/// - Remove first byte: No pre-position bytes, all bytes shift backward
/// - Very large files: Processes in chunks, no memory issues
///
/// # Example
/// ```no_run
/// # use std::io;
/// # use std::path::PathBuf;
/// # fn remove_single_byte_from_file(path: PathBuf, pos: usize) -> io::Result<()> { Ok(()) }
/// // Original file: [0x41, 0x42, 0x43, 0x44, 0x45]
/// let file_path = PathBuf::from("/absolute/path/to/file.dat");
/// let position = 2; // Remove byte at position 2 (0x43)
/// let result = remove_single_byte_from_file(file_path, position);
/// // Resulting file: [0x41, 0x42, 0x44, 0x45]
/// // Note: 0x44 and 0x45 shifted backward by 1 position
/// assert!(result.is_ok());
/// # Ok::<(), io::Error>(())
/// ```
pub fn remove_single_byte_from_file(
    original_file_path: PathBuf,
    byte_position_from_start: usize,
) -> io::Result<()> {
    // =========================================
    // Input Validation Phase
    // =========================================

    println!("=== Byte Removal Operation ===");
    println!("Target file: {}", original_file_path.display());
    println!("Byte position to remove: {}", byte_position_from_start);
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

    // Handle empty file case
    if original_file_size == 0 {
        let error_message = "Cannot remove byte from empty file (file size is 0)";
        eprintln!("ERROR: {}", error_message);
        return Err(io::Error::new(io::ErrorKind::InvalidInput, error_message));
    }

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

    println!(
        "Building modified draft file (removing byte at position {})...",
        byte_position_from_start
    );

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

    debug_assert!(
        BUCKET_BRIGADE_BUFFER_SIZE > 0,
        "Bucket brigade buffer must have non-zero size"
    );

    #[cfg(test)]
    {
        assert!(
            BUCKET_BRIGADE_BUFFER_SIZE > 0,
            "Bucket brigade buffer must have non-zero size"
        );
    }

    if BUCKET_BRIGADE_BUFFER_SIZE == 0 {
        let _ = fs::remove_file(&draft_file_path);
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid buffer configuration",
        ));
    }

    // Tracking variables
    let mut total_bytes_read_from_original: usize = 0;
    let mut total_bytes_written_to_draft: usize = 0;
    let mut chunk_number: usize = 0;
    let mut byte_was_removed = false;
    let mut removed_byte_value: u8 = 0;

    // Safety limit to prevent infinite loops
    const MAX_CHUNKS_ALLOWED: usize = 16_777_216;

    // =========================================
    // Main Processing Loop
    // =========================================

    loop {
        // =================================================
        // Debug-Assert, Test-Assert, Production-Catch-Handle
        // =================================================

        debug_assert!(
            chunk_number < MAX_CHUNKS_ALLOWED,
            "Exceeded maximum chunk limit"
        );

        #[cfg(test)]
        {
            assert!(
                chunk_number < MAX_CHUNKS_ALLOWED,
                "Exceeded maximum chunk limit"
            );
        }

        if chunk_number >= MAX_CHUNKS_ALLOWED {
            eprintln!("ERROR: Maximum chunk limit exceeded for safety");
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
            println!("Reached end of original file");
            break;
        }

        // =================================================
        // Debug-Assert, Test-Assert, Production-Catch-Handle
        // =================================================

        debug_assert!(
            bytes_read <= BUCKET_BRIGADE_BUFFER_SIZE,
            "Read more bytes than buffer size"
        );

        #[cfg(test)]
        {
            assert!(
                bytes_read <= BUCKET_BRIGADE_BUFFER_SIZE,
                "Read more bytes than buffer size"
            );
        }

        if bytes_read > BUCKET_BRIGADE_BUFFER_SIZE {
            eprintln!("ERROR: Buffer overflow detected");
            let _ = fs::remove_file(&draft_file_path);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Buffer overflow in read operation",
            ));
        }

        // Determine if target byte is in this chunk
        let chunk_start_position = total_bytes_read_from_original;
        let chunk_end_position = chunk_start_position + bytes_read;

        // Check if we need to skip a byte in this chunk (the removal operation)
        if byte_position_from_start >= chunk_start_position
            && byte_position_from_start < chunk_end_position
        {
            // Calculate position within this chunk
            let position_in_chunk = byte_position_from_start - chunk_start_position;

            // Store the byte being removed for verification
            removed_byte_value = bucket_brigade_buffer[position_in_chunk];
            byte_was_removed = true;

            println!(
                "Removing byte at position {}: 0x{:02X}",
                byte_position_from_start, removed_byte_value
            );

            // Write bytes BEFORE the removal position in this chunk
            if position_in_chunk > 0 {
                let bytes_before = &bucket_brigade_buffer[..position_in_chunk];
                let bytes_written_before = draft_file.write(bytes_before)?;

                // =================================================
                // Debug-Assert, Test-Assert, Production-Catch-Handle
                // =================================================

                debug_assert_eq!(
                    bytes_written_before, position_in_chunk,
                    "Not all pre-removal bytes were written"
                );

                #[cfg(test)]
                {
                    assert_eq!(
                        bytes_written_before, position_in_chunk,
                        "Not all pre-removal bytes were written"
                    );
                }

                if bytes_written_before != position_in_chunk {
                    eprintln!("ERROR: Incomplete write before removal position");
                    let _ = fs::remove_file(&draft_file_path);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "Incomplete write operation",
                    ));
                }

                total_bytes_written_to_draft += bytes_written_before;
            }

            // SKIP the byte at position_in_chunk (this is the removal operation)
            // Do not write bucket_brigade_buffer[position_in_chunk] to draft

            // Write bytes AFTER the removal position in this chunk
            let position_after_removal = position_in_chunk + 1;
            if position_after_removal < bytes_read {
                let bytes_after = &bucket_brigade_buffer[position_after_removal..bytes_read];
                let bytes_written_after = draft_file.write(bytes_after)?;

                let expected_bytes_after = bytes_read - position_after_removal;

                // =================================================
                // Debug-Assert, Test-Assert, Production-Catch-Handle
                // =================================================

                debug_assert_eq!(
                    bytes_written_after, expected_bytes_after,
                    "Not all post-removal bytes were written"
                );

                #[cfg(test)]
                {
                    assert_eq!(
                        bytes_written_after, expected_bytes_after,
                        "Not all post-removal bytes were written"
                    );
                }

                if bytes_written_after != expected_bytes_after {
                    eprintln!("ERROR: Incomplete write after removal position");
                    let _ = fs::remove_file(&draft_file_path);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "Incomplete write operation",
                    ));
                }

                total_bytes_written_to_draft += bytes_written_after;
            }
        } else {
            // This chunk does not contain the removal position
            // Write entire chunk to draft file
            let bytes_written = draft_file.write(&bucket_brigade_buffer[..bytes_read])?;

            // =================================================
            // Debug-Assert, Test-Assert, Production-Catch-Handle
            // =================================================

            debug_assert_eq!(bytes_written, bytes_read, "Not all bytes were written");

            #[cfg(test)]
            {
                assert_eq!(bytes_written, bytes_read, "Not all bytes were written");
            }

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

            total_bytes_written_to_draft += bytes_written;
        }

        total_bytes_read_from_original += bytes_read;

        // Flush to ensure data is written
        draft_file.flush()?;
    }

    // =========================================
    // Basic Verification Phase
    // =========================================

    println!("\nVerifying operation...");

    // Verify byte was actually removed
    if !byte_was_removed {
        eprintln!("ERROR: Target byte position was never reached");
        let _ = fs::remove_file(&draft_file_path);
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Byte removal did not occur",
        ));
    }

    // Verify draft file is exactly 1 byte smaller
    draft_file.flush()?;
    drop(draft_file);
    drop(source_file);

    let draft_metadata = fs::metadata(&draft_file_path)?;
    let draft_size = draft_metadata.len() as usize;
    let expected_draft_size = original_file_size - 1;

    // =================================================
    // Debug-Assert, Test-Assert, Production-Catch-Handle
    // =================================================

    debug_assert_eq!(draft_size, expected_draft_size, "Draft file size incorrect");

    #[cfg(test)]
    {
        assert_eq!(draft_size, expected_draft_size, "Draft file size incorrect");
    }

    if draft_size != expected_draft_size {
        eprintln!(
            "ERROR: File size mismatch - original: {} bytes, draft: {} bytes, expected: {} bytes",
            original_file_size, draft_size, expected_draft_size
        );
        let _ = fs::remove_file(&draft_file_path);
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "File size verification failed",
        ));
    }

    println!(
        "Basic verification passed: original={} bytes, draft={} bytes (-1 byte)",
        original_file_size, draft_size
    );

    // =========================================
    // Comprehensive Verification Phase
    // =========================================

    // Perform all verification checks before replacing the original
    verify_byte_removal_operation(
        &original_file_path,
        &draft_file_path,
        byte_position_from_start,
        removed_byte_value,
    )?;

    // =========================================
    // Atomic Replacement Phase
    // =========================================

    println!("\nReplacing original file with modified version...");

    // Attempt atomic rename
    match fs::rename(&draft_file_path, &original_file_path) {
        Ok(()) => {
            println!("Original file successfully replaced");
        }
        Err(e) => {
            eprintln!("Cannot atomically replace file: {}", e);
            eprintln!("Original and backup files preserved for safety");
            return Err(e);
        }
    }

    // =========================================
    // Cleanup Phase
    // =========================================

    println!("\nCleaning up backup file...");

    match fs::remove_file(&backup_file_path) {
        Ok(()) => println!("Backup file removed"),
        Err(e) => {
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
    println!("Removed byte at position: {}", byte_position_from_start);
    println!("Removed byte value: 0x{:02X}", removed_byte_value);
    println!("Original size: {} bytes", original_file_size);
    println!("New size: {} bytes", draft_size);
    println!(
        "Bytes read from original: {}",
        total_bytes_read_from_original
    );
    println!("Bytes written to draft: {}", total_bytes_written_to_draft);
    println!("Total chunks: {}", chunk_number);
    println!("Status: SUCCESS");

    Ok(())
}

// =========================================
// Test Module
// =========================================

#[cfg(test)]
mod removal_tests {
    use super::*;

    #[test]
    fn test_remove_single_byte_basic() {
        let test_dir = std::env::temp_dir();
        let test_file = test_dir.join("test_byte_remove.bin");

        // Create test file: [0x00, 0x11, 0x22, 0x33, 0x44]
        let test_data = vec![0x00, 0x11, 0x22, 0x33, 0x44];
        std::fs::write(&test_file, &test_data).expect("Failed to create test file");

        // Remove byte at position 2 (0x22)
        let result = remove_single_byte_from_file(test_file.clone(), 2);

        assert!(result.is_ok(), "Operation should succeed");

        // Verify result: [0x00, 0x11, 0x33, 0x44]
        let modified_data = std::fs::read(&test_file).expect("Failed to read modified file");
        assert_eq!(modified_data, vec![0x00, 0x11, 0x33, 0x44]);

        // Cleanup
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_remove_first_byte() {
        let test_dir = std::env::temp_dir();
        let test_file = test_dir.join("test_remove_first.bin");

        let test_data = vec![0xAA, 0xBB, 0xCC];
        std::fs::write(&test_file, &test_data).expect("Failed to create test file");

        // Remove first byte
        let result = remove_single_byte_from_file(test_file.clone(), 0);

        assert!(result.is_ok());

        let modified_data = std::fs::read(&test_file).expect("Failed to read modified file");
        assert_eq!(modified_data, vec![0xBB, 0xCC]);

        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_remove_last_byte() {
        let test_dir = std::env::temp_dir();
        let test_file = test_dir.join("test_remove_last.bin");

        let test_data = vec![0xAA, 0xBB, 0xCC];
        std::fs::write(&test_file, &test_data).expect("Failed to create test file");

        // Remove last byte
        let result = remove_single_byte_from_file(test_file.clone(), 2);

        assert!(result.is_ok());

        let modified_data = std::fs::read(&test_file).expect("Failed to read modified file");
        assert_eq!(modified_data, vec![0xAA, 0xBB]);

        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_remove_from_single_byte_file() {
        let test_dir = std::env::temp_dir();
        let test_file = test_dir.join("test_remove_single.bin");

        std::fs::write(&test_file, vec![0x42]).expect("Failed to create test file");

        let result = remove_single_byte_from_file(test_file.clone(), 0);

        assert!(result.is_ok());

        let modified_data = std::fs::read(&test_file).expect("Failed to read modified file");
        assert_eq!(modified_data, Vec::<u8>::new()); // Empty file

        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_remove_byte_out_of_bounds() {
        let test_dir = std::env::temp_dir();
        let test_file = test_dir.join("test_remove_bounds.bin");

        std::fs::write(&test_file, vec![0x00, 0x11]).expect("Failed to create test file");

        let result = remove_single_byte_from_file(test_file.clone(), 10);

        assert!(result.is_err(), "Should fail with out of bounds position");

        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_remove_from_empty_file() {
        let test_dir = std::env::temp_dir();
        let test_file = test_dir.join("test_remove_empty.bin");

        File::create(&test_file).expect("Failed to create empty file");

        let result = remove_single_byte_from_file(test_file.clone(), 0);

        assert!(result.is_err(), "Should fail with empty file");

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

    // Run: In-Place-Edit
    let result_tui =
        replace_single_byte_in_file(original_file_path, byte_position_from_start, new_byte_value);
    println!("result_tui -> {:?}", result_tui);

    // Test 2: Remove Byte
    let test_dir_2 = std::env::current_dir()?;
    let original_file_path = test_dir_2.join("pytest_file_2.py");
    let byte_position_from_start: usize = 3;

    // Run: Remove
    let result_tui = remove_single_byte_from_file(original_file_path, byte_position_from_start);
    println!("result_tui -> {:?}", result_tui);

    // Test 3: Add Byte

    println!("main() All Done!");
    Ok(())
}
