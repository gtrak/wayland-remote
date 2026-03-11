# Code Review: Phase 05 - Plan 01 (TCP Client Foundation)

**Review Cycle:** 1/5
**Date:** 2026-03-11

## Previous Issues Status

*No previous issues to track (first review cycle)*

## Current Issues

### Minor

- **[m-1]** Incorrect documentation for `connect()` method
  - **Location:** `crates/viewer/src/network/client.rs:29-35`
  - **Issue:** Doc comment mentions a parameter `rx` that doesn't exist in the function signature. The actual function takes no parameters and returns a `Result<TcpStream>`.
  - **Severity:** Minor
  - **Category:** Code Quality
  - **Fix:** Update documentation to accurately reflect the function signature:
    ```rust
    /// Connect to the server
    ///
    /// # Returns
    /// Result containing the connected TcpStream on success
    ```

- **[m-2]** Unnecessary async on `start_receiving` method
  - **Location:** `crates/viewer/src/network/client.rs:72-98`
  - **Issue:** The `start_receiving` method is marked `async` but performs no await operations before spawning the background task. It immediately creates a channel, spawns a task, and returns the receiver. This could be a synchronous method.
  - **Severity:** Minor
  - **Category:** Code Quality
  - **Fix:** Remove `async` keyword from method signature:
    ```rust
    pub fn start_receiving(
        stream: TcpStream,
        buffer_size: usize,
    ) -> mpsc::Receiver<Frame>
    ```

- **[m-3]** Hardcoded payload size limit
  - **Location:** `crates/viewer/src/network/client.rs:127-132`
  - **Issue:** The 100MB payload size limit is hardcoded. While this prevents DoS attacks, it may be too restrictive for high-resolution displays (e.g., 4K at 60fps with multi-monitor setups could exceed this).
  - **Severity:** Minor
  - **Category:** Best Practices
  - **Fix:** Make the limit configurable via a constant or builder pattern:
    ```rust
    pub const DEFAULT_MAX_PAYLOAD_SIZE: usize = 100_000_000;
    
    pub struct TcpClient {
        address: String,
        max_payload_size: usize,
    }
    ```

- **[m-4]** Missing reconnection logic
  - **Location:** `crates/viewer/src/network/client.rs:78-95`
  - **Issue:** The risk mitigation table in the plan mentions "Implement reconnection logic, error propagation" but the current implementation simply breaks the loop on any error. This is acceptable for the foundation but should be implemented before production use.
  - **Severity:** Minor
  - **Category:** Partial Implementation
  - **Fix:** Document that reconnection is TODO for future plans, or implement exponential backoff reconnection.

- **[m-5]** Doc comment parameter mismatch in `read_frame`
  - **Location:** `crates/viewer/src/network/client.rs:47-59`
  - **Issue:** Doc comment mentions "Reads the 20-byte header first, then reads the payload" but the method delegates immediately to `read_frame_from_stream`. The documentation is slightly misleading about where the logic lives.
  - **Severity:** Minor
  - **Category:** Code Quality
  - **Fix:** Update doc comment to clarify that this is a convenience wrapper.

## Summary

| Status | Critical | Major | Minor |
|--------|----------|-------|-------|
| Previous Fixed | 0 | 0 | 0 |
| Previous Remaining | 0 | 0 | 0 |
| New | 0 | 0 | 5 |
| **Total Open** | 0 | 0 | 5 |

**Previous Issues:** 0 fixed, 0 remaining
**New Issues:** 0 critical, 0 major, 5 minor
**Status:** ✅ ISSUES_RESOLVED (all issues are minor documentation/code quality)

## Positive Findings

1. **Protocol Implementation**: The 20-byte big-endian header parsing is correctly implemented with proper error handling for insufficient data.

2. **Test Coverage**: 15 unit tests covering protocol parsing, encoding/decoding round-trips, frame validation, error cases (incomplete header/payload), and big-endian byte order verification.

3. **Safety**: No unsafe code blocks. All unwrap calls are confined to test code.

4. **Async Pattern**: Correct use of Tokio's `read_exact` for atomic frame reading, preventing partial frame issues.

5. **Resource Management**: Proper use of mpsc channels for thread-safe frame delivery without blocking UI.

6. **Error Handling**: Comprehensive NetworkError enum with thiserror derive providing clean error propagation.

7. **Payload Sanity Check**: 100MB limit prevents memory exhaustion from malicious frame headers.

## Verification

All requirements from PLAN.md verified:
- ✅ TCP client connects to server on configurable address:port
- ✅ 20-byte frame header parsed correctly (window_id, width, height, timestamp)
- ✅ RGBA payload read based on header dimensions
- ✅ Network thread spawned separately (via Tokio task) from UI thread
- ✅ Frames sent to main thread via channel for display

All 15 tests pass:
```
test network::client::tests::test_client_address_parsing ... ok
test network::client::tests::test_client_creation ... ok
test network::client::tests::test_connection_refused ... ok
test network::protocol::tests::test_big_endian_ordering ... ok
test network::protocol::tests::test_decode_insufficient_data ... ok
test network::client::tests::test_read_frame_from_mock_server ... ok
test network::protocol::tests::test_header_decode ... ok
test network::client::tests::test_read_frame_incomplete_header ... ok
test network::protocol::tests::test_frame_validity ... ok
test network::protocol::tests::test_header_encode ... ok
test network::client::tests::test_read_frame_incomplete_payload ... ok
test network::protocol::tests::test_header_total_size ... ok
test network::protocol::tests::test_header_payload_size ... ok
test network::tests::test_frame_header_wire_size ... ok
test network::tests::test_frame_struct_exists ... ok
```

---
*Reviewed by: gsd-code-reviewer | Cycle: 1/5*
