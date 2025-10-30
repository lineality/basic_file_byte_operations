#### basic_file_byte_operations

This file-byte operation project is an intersection of several projects and primarily designed one other project-set, and should ideally be a general module that can be cleanly used in various projects.

### Overall:
- making byte-level changes to a file
- operates are single byte operations
- a byte is 8 bits
- should follow assorted best practice, see below

#### To Use:
- Bucket Brigade - no-load write writing
- (maybe) Ribbon - no-load counting
- Read-Copy, Backup-Copy - safe file handling

#### For Projects:
- Reversible File-Edit Changelog
- Hex Editor: managing add and remove byte operations safely.

# Three Operations:
1. hex_edit_byte_operation:
- requires position of byte from start of file
- change one byte
- no change to file length
- no insertion or deletion of bytes
- no frame shift in series of file bytes

2. remove_byte_operation
- requires position of byte from start of file
- remote one byte at that position, remove that position
- the byte that was one after that position is now at that position
- result is no change before that position
- from that position is frame-shifted -1 (a bit-shift, if you will) towards file-start

3. add_byte_at_position_operation
- requires position of byte from start of file
- and one byte at that position
- the byte that was at that position is now one ahead (+1)
- result is no change before that position
- after the position is +1 frame-shifted (a bit-shift, if you will) away from file-start

## File handling 1: Safe Copies & Backups
- the original file is not change/replaced until the ~last step (not including cleanup and ending).
- a backup is made of the original and not removed until the ~end of the process.
- a safety-copy of the original file is made to operate on, one way or other (see more below)

## File Handling 2: No-Load Operations
- small pre-allocated buffers only for production-release build code

## Error and Exception Handling
- small pre-allocated buffers only for production-release build code


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
- (double)checking original vs. new file: pre-position byte length similarity (possible a hash-check) Note: if the orignal file is empty (zero bytes), or one single byte, or if the target-position is the first byte, then the length-similiarity distance for this step is zero.
- (double)checking original vs. new file value: at-position: curious edge cases, this is a two part check. In each case 'new' value may be the same as the old. The first check to so compare the new value (different source for each operation) to see if it is the same as old. If not, it can be check for being different. (This step may be considered an extra-extra-check that not everyone would think they need.)
- (double)checking original vs. new file: post-position, must be similarity given frame-shift or not (possible a hash-check)
 - - hex-edit in place: no frameshift: post-position must be the same
 - - remove byte: -1 frameshift in new file compared with original: given -1 frameshift post position must be the same
 - - add byte: +1 from target position frameshift in new file compared with original file at target-position: given the +1 frameshift, post position data must be the same. Example of verification with +1 frame-shift after adding byte at position N:
 ```
 draft[N+1] == original[N] (first byte after insertion)
 draft[N+2] == original[N+1] (second byte after insertion)
 ```

- For byte-addition only there could be an additional check: checking if the add position byte is the new byte (this is not a file comparison check).

# Bucket Brigade

It may be possible to (in addition to more mature error/exception handling etc.) adapt this proof of concept (POC) Bucket Bridage code to:
- make backup copy and use draft-copy
- build the draft in three steps (described above)
- run test-checks (described above)
- replace the original file
- remove backup-copy (if no-issues)
- return exit code, etc. Finish
#### for in-place-byte-edit:
- take inputs of byte_position, new byte, instead of a second file path
#### for remove-byte-edit:
- take input of byte_position, instead of a second file path
#### for add-byte-edit:
- take inputs of byte_position, new byte, instead of a second file path

### recommended
- three do one thing well functions vs. 1 swiss-army-knife

```
# Bucket Brigade pre-allocated processing of stdin

POC programs to read input iteratively (in chunks)
over and including multiple newlines
(given the edge case of the leftover pizza problem)
using a pre-allocated buffer.

## This scope includes:
- must use pre-allocated buffer
- must not use heap
- must not use read_line()
- must not halt at first newline
- stdin input can/will contain multiple newlines (e.g. cut and past multiple lines)
- must handle all newlines *in the input
  -- user must enter 'Enter' key if multiline does not end in newline
- must never load all of input into memory at once, only one chunk

# Requires exit signal
A kind of ~halting problem: we cannot know the state of stdin
Use a specific exit command: -q -n -v
when changing mode or quitting, etc.
stdin is a process that has no end to predict.

This:
```
if bytes_read == 0 {
    println!("bytes_read == 0");
    break;
}
```
is never triggered (or would never be triffered).
stdin requests more input.
The program never "finishes."

# The Leftover Pizza Problem

stdin sometimes has "left over pizza,"
a problem where (for whatever reasons)
a newline does not appear at the end text entry,
and is in stdin,
so that content is stuck
until there is another addition.

Because of the riddle of ~blocked stdin
and not being able to tell when it is empty,
or being able to guarantee that it contains
input followed by the \n newline that it needs,
and the side effect of it sometimes asking
for more text before it is empty:

If there is multiline input that does not end in a newline,
the user needs to press the Enter key one more time,
even though the multi-line input did go into stdin,
e.g. there is no lingering text in the terminal waitng
for the user to hit enter.
```
// bucket_brigade_preallocated_chunking.rs
use std::{
    env,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::PathBuf,
};

/// Reads a file in pre-allocated chunks and appends content to another file.
///
/// # Memory Safety
/// - Uses pre-allocated 64-byte buffer (no heap allocation)
/// - Never loads entire source or destination file into memory
/// - Processes file chunk-by-chunk using bucket brigade pattern
///
/// # File Behavior
/// - Opens source file in read-only mode
/// - Opens destination file in append mode (creates if doesn't exist)
/// - Reads sequentially from source start to EOF
/// - Appends each chunk immediately to destination (no buffering)
/// - Flushes after each write for durability
/// - Both files closed automatically when function exits
///
/// # Use Cases
/// - Combining log files
/// - Appending data without loading entire files
/// - Safe file concatenation for large files
/// - Incremental backups
///
/// # Edge Cases
/// - Empty source file: creates/touches destination, writes 0 bytes (valid operation)
/// - Source file not found: returns io::Error
/// - Destination doesn't exist: creates new file
/// - Destination exists: appends to end (preserves existing content)
/// - Source and destination are same file: allowed but NOT RECOMMENDED (will double content)
/// - Very large files: chunked processing prevents memory exhaustion
/// - Filesystem full: returns io::Error on write
///
/// # Safety Limits
/// - Maximum chunks: 16,777,216 (allows ~1GB at 64-byte chunks)
/// - Prevents infinite loops from filesystem corruption or cosmic ray errors
///
/// # Parameters
/// - `from_path`: Absolute path to source file to read from
/// - `to_path`: Absolute path to destination file to append to
///
/// # Returns
/// - `Ok(())` on successful copy
/// - `Err(io::Error)` if source cannot be opened, destination cannot be written, or read/write fails
///
/// # Example
/// ```no_run
/// # use std::io;
/// # use std::path::PathBuf;
/// # use std::env;
/// # fn file_append_to_file(from_path: PathBuf, to_path: PathBuf) -> io::Result<()> { Ok(()) }
/// let current_dir = env::current_dir()?;
/// let source_path = current_dir.join("demo.txt");
/// let dest_path = current_dir.join("append_to_this.txt");
/// let result = file_append_to_file(source_path, dest_path);
/// assert!(result.is_ok());
/// # Ok::<(), io::Error>(())
/// ```
fn file_append_to_file(from_path: PathBuf, to_path: PathBuf) -> io::Result<()> {
    // use std::fs::{File, OpenOptions};
    // use std::io::Write;

    println!("=== File to File Append ===\n");
    println!("Reading from: {}", from_path.display());
    println!("Appending to: {}\n", to_path.display());

    // Defensive: Check source file exists before attempting operations
    if !from_path.exists() {
        eprintln!("ERROR: Source file does not exist: {}", from_path.display());
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Source file not found: {}", from_path.display()),
        ));
    }

    // Defensive: Warn if source and destination are the same file
    // (This is allowed but dangerous - will double the file content)
    if from_path == to_path {
        eprintln!("WARNING: Source and destination are the same file!");
        eprintln!("This will append file to itself, doubling its content.");
        eprintln!("Proceeding anyway, but this is likely not intended.\n");
    }

    // Open source file in read-only mode
    let mut source_file = File::open(&from_path)?;

    // Open destination file in append mode (create if doesn't exist)
    let mut dest_file = OpenOptions::new()
        .create(true) // Create file if it doesn't exist
        .append(true) // Append to existing content
        .open(&to_path)?;

    // Pre-allocated buffer for bucket brigade processing
    const SIZE_OF_BUCKET_BRIGADE_BUFFER: usize = 64;
    let mut main_bucket_brigade_buffer = [0u8; SIZE_OF_BUCKET_BRIGADE_BUFFER];

    // Counters for diagnostic feedback
    let mut chunk_number = 0;
    let mut total_bytes_processed = 0;

    // Safety: Maximum iterations to prevent infinite loop
    // Allows 1GB of file at 64-byte chunks = ~16 million chunks
    const MAX_CHUNKS: usize = 16_777_216;

    loop {
        // Defensive: prevent infinite loop from filesystem corruption or cosmic ray
        if chunk_number >= MAX_CHUNKS {
            eprintln!(
                "ERROR: Maximum chunk limit reached ({}). Exiting for safety.",
                MAX_CHUNKS
            );
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Maximum iteration limit exceeded",
            ));
        }

        // Clear buffer before reading (defensive: prevent data leakage between reads)
        for i in 0..SIZE_OF_BUCKET_BRIGADE_BUFFER {
            main_bucket_brigade_buffer[i] = 0;
        }

        chunk_number += 1;

        // Read next chunk from source file
        let bytes_read = source_file.read(&mut main_bucket_brigade_buffer)?;

        // Defensive assertion: bytes_read should never exceed buffer size
        assert!(
            bytes_read <= SIZE_OF_BUCKET_BRIGADE_BUFFER,
            "bytes_read ({}) exceeded buffer size ({})",
            bytes_read,
            SIZE_OF_BUCKET_BRIGADE_BUFFER
        );

        // EOF detection: bytes_read == 0 reliably signals end of source file
        if bytes_read == 0 {
            println!("[End of source file reached]");
            break;
        }

        // Write chunk to destination file (never buffer entire content in memory)
        let bytes_written = dest_file.write(&main_bucket_brigade_buffer[..bytes_read])?;

        // Defensive assertion: all bytes should be written
        assert_eq!(
            bytes_written, bytes_read,
            "Destination write incomplete: wrote {} of {} bytes",
            bytes_written, bytes_read
        );

        // Flush to disk immediately for durability (survive crashes/power loss)
        dest_file.flush()?;

        total_bytes_processed += bytes_written;

        println!(
            "Chunk {}: copied {} bytes (total: {})",
            chunk_number, bytes_written, total_bytes_processed
        );

        // Optional diagnostic: show content if valid UTF-8
        if let Ok(s) = std::str::from_utf8(&main_bucket_brigade_buffer[..bytes_read]) {
            println!("  Content: {:?}", s);
        }
    }

    // Final flush to ensure all data is on disk
    dest_file.flush()?;

    // Handle empty file case
    if total_bytes_processed == 0 {
        println!("Source file was empty (0 bytes copied).");
    }

    println!("\n=== Summary ===");
    println!("Total bytes copied: {}", total_bytes_processed);
    println!("Total chunks: {}", chunk_number - 1); // Subtract 1 because last iteration hit EOF
    println!("Source: {}", from_path.display());
    println!("Destination: {}", to_path.display());
    println!("All Done!");

    Ok(())
}


# Ribbon
Because this project (byte operations) focuses on small steps, there may not be a need here to count potentially larger than variable-size quantities.
It is possible to manage this without.






# Policies and Rules
```
# Rust rules:
- Always best practice.
- Always extensive doc strings.
- Always comments.
- Always cargo tests (where possible).
- Never remove documentation.
- Always clear, meaningful, unique names (e.g. variables, functions).
- Always absolute file paths.
- Always error handling.
- Never unsafe code.
- Never use unwrap.

- Load what is needed when it is needed: Do not ever load a whole file or line, rarely load a whole anything. increment and load only what is required pragmatically. Do not fill 'state' with every possible piece of un-used information. Do not insecurity output information broadly in the case of errors and exceptions.

- Always defensive best practice
- Always error handling: Every part of code, every process, function, and operation will fail at some point, if only because of cosmic-ray bit-flips (which are common), hardware failure, power-supply failure, adversarial attacks, etc. There must always be fail-safe error handling where production-release-build code handles issues and moves on without panic-crashing ever. Every failure must be handled smoothly: let it fail and move on.


Safety, reliability, maintainability, fail-safe, communication-documentation, are the goals: not ideology, aesthetics, popularity, momentum-tradition, bad habits, convenience, nihilism, lazyness, lack of impulse control, etc.

## No third party libraries (or very strictly avoid third party libraries where possible).

## Rule of Thumb, ideals not absolute rules: Follow NASA's 'Power of 10 rules' where possible and sensible (as updated for 2025 and Rust (not narrowly 2006 c for embedded systems):
1. no unsafe stuff:
- no recursion
- no goto
- no pointers
- no preprocessor

2. upper bound on all normal-loops, failsafe for all always-loops

3. Pre-allocate all memory (no dynamic memory allocation)

4. Clear function scope and Data Ownership: Part of having a function be 'focused' means knowing if the function is in scope. Functions should be neither swiss-army-knife functions that do too many things, nor scope-less micro-functions that may be doing something that should not be done. Many functions should have a narrow focus and a short length, but definition of actual-project scope functionality must be explicit. Replacing one long clear in-scope function with 50 scope-agnostic generic sub-functions with no clear way of telling if they are in scope or how they interact (e.g. hidden indirect recursion) is unsafe. Rust's ownership and borrowing rules focus on Data ownership and hidden dependencies, making it even less appropriate to scatter borrowing and ownership over a spray of microfunctions purely for the ideology of turning every operation into a microfunction just for the sake of doing so. (See more in rule 9.)

5. Defensive programming: debug-assert, test-assert, prod safely check & handle, not 'assert!' panic
For production-release code:
1. check and handle without panic/halt in production
2. return result (such as Result<T, E>) and smoothly handle errors (not halt-panic stopping the application): no assert!() outside of test-only code
3. test assert: use #[cfg(test)] assert!() to test production binaries (not in prod)
4. debug assert: use debug_assert to test debug builds/runs (not in prod)
5. use defensive programming with recovery of all issues at all times
- use cargo tests
- use debug_asserts
- do not leave assertions in production code.
- use no-panic error handling
- use Option
- use enums and structs
- check bounds
- check returns
- note: a test-flagged assert can test a production release build (whereas debug_assert cannot); cargo test --release
```
#[cfg(test)]
assert!(
```

e.g.
# "Assert & Catch-Handle" 3-part System

// template/example for check/assert format
//    =================================================
// // Debug-Assert, Test-Asset, Production-Catch-Handle
//    =================================================
// This is not included in production builds
// assert: only when running in a debug-build: will panic
debug_assert!(
    INFOBAR_MESSAGE_BUFFER_SIZE > 0,
    "Info bar buffer must have non-zero capacity"
);
// This is not included in production builds
// assert: only when running cargo test: will panic
#[cfg(test)]
assert!(
    INFOBAR_MESSAGE_BUFFER_SIZE > 0,
    "Info bar buffer must have non-zero capacity"
);
// Catch & Handle without panic in production
// This IS included in production to safe-catch
if !INFOBAR_MESSAGE_BUFFER_SIZE == 0 {
    // state.set_info_bar_message("Config error");
    return Err(LinesError::GeneralAssertionCatchViolation(
        "zero buffer size error".into(),
    ));
}


Avoid heap for error messages and for all things:
Is heap used for error messages because that is THE best way, the most secure, the most efficient, proper separate of debug testing vs. secure production code?
Or is heap used because of oversights and apathy: "it's future dev's problem, let's party."
We can use heap in debug/test modes/builds only.
Production software must not insecurely output debug diagnostics.
Debug information must not be included in production builds: "developers accidentally left development code in the software" is a classic error (not a desired design spec) that routinely leads to security and other issues. That is NOT supposed to happen. It is not coherent to insist the open ended heap output 'must' or 'should' be in a production build.

This is central to the question about testing vs. a pedantic ban on conditional compilation; not putting full traceback insecurity into production code is not a different operational process logic tree for process operations.

Just like with the pedantic "all loops being bounded" rule, there is a fundamental exception: always-on loops must be the opposite.
With conditional compilations: code NEVER to EVER be in production-builds MUST be always "conditionally" excluded. This is not an OS conditional compilation or a hardware conditional compilation. This is an 'unsafe-testing-only or safe-production-code' condition.

Error messages and error outcomes in 'production' 'release' (real-use, not debug/testing) must not ever contain any information that could be a security vulnerability or attack surface. Failing to remove debugging inspection is a major category of security and hygiene problems.

Security: Error messages in production must NOT contain:
- File paths (can reveal system structure)
- File contents
- environment variables
- user, file, state, data
- internal implementation details
- etc.

Production output following an error must be managed and defined, not not open to whatever an api or OS-call wants to dump out.

6. Manage ownership and borrowing

7. Manage return values:
- use null-void return values
- check non-void-null returns

8. Navigate debugging and testing on the one hand and not-dangerous conditional compilation on the other hand

9. Communicate:
- use doc strings, use comments,
- Document use-cases, edge-cases, and policies (These are project specific and cannot be telepathed from generic micro-function code. When a Mars satellite failed because one team used SI-metric units and another team did not, that problem could not have been detected by looking at, and auditing, any individual function in isolation without documentation. Breaking a process into innumerable undocumented micro-functions can make scope and policy impossible to track. To paraphrase Jack Welch: "The most dangerous thing in the world is a flawless operation that should never have been done in the first place.")

10. Use state-less operations when possible:
- a seemingly invisibly small increase in state often completely destroys projects
- expanding state destroys projects with unmaintainable over-reach

Vigilance: We should help support users and developers and the people who depend upon maintainable software. Maintainable code supports the future for us all.
```
