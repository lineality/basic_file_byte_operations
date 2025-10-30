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
- from that position is frame-shifted (a bit-shift, if you will) towards file-start

3. add_byte_at_position_operation
- requires position of byte from start of file
- and one byte at that position
- the byte that was at that position is not one ahead
- result is no change before that position
- after the position is frame-shifted (a bit-shift, if you will) away from file-start

## File handling 1: Safe Copies & Backups
- the original file is not change/replaced until the ~last step (not including cleanup and ending).
- a backup is made of the original and not removed until the ~end of the process.
- a safety-copy of the original file is made to operate on, one way or other (see more below)

## File Handling 2: No-Load Operations
- small pre-allocated buffers only for production-release build code

## Error and Exception Handling
- small pre-allocated buffers only for production-release build code


# File Identities
At the granular level of these operations, it may be best to avoid user-abstractions such as 'add' or 'remove' or 'modify' and 'original' or 'copy'
and to look instead at specific steps and actions.

It may be possible to effect the desired end-state (retroacively described as 'add' or 'remove') with steps such as these:
1. Create a draft file.
2. Append bytes (from the original file, to the draft-file) up to the 'file byte position of the change operation' in question:
append byte by byte, or append with a small bucket-brigade buffer.
3. Performing Operation at 'file byte position of the change operation':
- For Remove-a-byte-operation: 'planning action' frame shift/advance your reading of the original file one byte without writing to the draft.
- For Add-a-byte-operation: append the 'new' (not in original file) byte to the draft file. Do not shift original file read-location.
4. Append bytes (from the original file, to the draft-file) up to the 'file byte position of the change operation' in question:
append byte by byte, or append with a small bucket-brigade buffer.


In theory, this process only 'need' apply to Add-a-byte-operation and Remove-a-byte-operation not (hex-edit)change-a-byte-in-place. An in-place byte change can be done simple on a file. However, what is better:
1. A standard process of building a new file cleanly and not making any internal changes to it and which is a single process always used, or
2. Having two different workflows in the same tool-kit, whereby in-place edit makes a complete copy of a file and then navigates back to the change-spot and changes it and resaves the file. Is that simpler than writing the file per-design in the first place with a standard workflow?

Using these steps we are not 'altering' any file per-se; we are constructing the 'altered' (relatively speaking) file in one clean workflow.

There can also be checking steps such as:
- (double)checking original vs. new file byte length
- (double)checking original vs. new file pre-position similarity (possible a hash-check)
- (double)checking original vs. new file post-position similarity (possible a hash-check)
- (double)checking original vs. new file at-position dissimilarity


# Bucket Brigade


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
