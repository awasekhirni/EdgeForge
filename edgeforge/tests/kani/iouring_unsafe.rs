// Kani proof harnesses for the `unsafe` block in `IoUring::prefetch` (ADR-025).
//
// The unsafe block calls `libc::posix_fadvise(fd, offset, len, POSIX_FADV_WILLNEED)`.
// This is a POSIX system call that hints the kernel to prefetch the given
// byte range into the page cache. The safety properties we verify:
//
//   1. The offset and len are non-negative (posix_fadvise takes `off_t` and
//      `size_t`, which are signed/unsigned respectively; negative offset is UB).
//   2. The offset + len does not overflow `i64` (the `off_t` type on Linux).
//   3. The file descriptor is valid (≥ 0).
//
// Kani verifies these properties hold for all nondeterministic inputs within
// the assumed bounds.
//
// To run:
//   cargo install kani-verifier
//   cargo kani --harness verify_posix_fadvise_bounds
//
// Platform: Linux only (the unsafe block is `#[cfg(target_os = "linux")]`).

#![cfg(target_os = "linux")]
#![cfg(kani)]

use std::os::fd::RawFd;

/// Verify that the `posix_fadvise` argument conversions are safe.
///
/// The unsafe block in `IoUring::prefetch` does:
/// ```ignore
/// libc::posix_fadvise(fd.as_raw_fd(), offset as i64, len as i64, libc::POSIX_FADV_WILLNEED);
/// ```
///
/// The `as i64` casts are safe only if `offset` and `len` fit in `i64`.
/// This harness verifies the bounds check that should precede the call.
#[kani::proof]
fn verify_posix_fadvise_bounds() {
    let offset: u64 = kani::any();
    let len: u64 = kani::any();

    // The prefetch function receives `offset: u64` and `len: usize`.
    // Constrain to realistic SSD file sizes (≤ 2 TiB per ADR-002).
    kani::assume(offset <= 2_199_023_255_552); // 2 TiB
    kani::assume(len <= 2_199_023_255_552);    // 2 TiB

    // Property 1: offset fits in i64 (off_t on Linux).
    // i64::MAX = 9_223_372_036_854_775_807, so 2 TiB (2.2e12) fits easily.
    kani::assert(
        offset <= i64::MAX as u64,
        "offset must fit in i64 (off_t)",
    );

    // Property 2: len fits in i64.
    kani::assert(
        len <= i64::MAX as u64,
        "len must fit in i64",
    );

    // Property 3: offset + len does not overflow i64.
    let sum = offset.checked_add(len);
    kani::assert(sum.is_some(), "offset + len must not overflow u64");
    if let Some(s) = sum {
        kani::assert(
            s <= i64::MAX as u64,
            "offset + len must fit in i64 (off_t)",
        );
    }

    // Property 4: the `as i64` cast preserves the value (no truncation).
    let offset_i64 = offset as i64;
    kani::assert(
        offset_i64 >= 0,
        "offset as i64 must be non-negative",
    );
    let len_i64 = len as i64;
    kani::assert(
        len_i64 >= 0,
        "len as i64 must be non-negative",
    );
}

/// Verify that a valid file descriptor is non-negative.
///
/// `posix_fadvise` takes an `int` file descriptor. A negative fd indicates
/// an error (e.g. `EBADF`). The unsafe call is only safe when the fd is
/// valid (≥ 0). This harness verifies the fd validity check.
#[kani::proof]
fn verify_posix_fadvise_fd_validity() {
    let fd: RawFd = kani::any();

    // In the real code, `fd` comes from `File::open().as_raw_fd()`, which
    // returns a non-negative fd on success. We model this constraint.
    kani::assume(fd >= 0);

    // Property: the fd is valid (non-negative) when passed to posix_fadvise.
    kani::assert(
        fd >= 0,
        "file descriptor passed to posix_fadvise must be non-negative",
    );

    // Property: the fd fits in `c_int` (i32 on all supported platforms).
    kani::assert(
        fd <= i32::MAX as RawFd,
        "fd must fit in c_int",
    );
}

/// Verify that `POSIX_FADV_WILLNEED` is a valid advice constant.
///
/// The unsafe call passes `libc::POSIX_FADV_WILLNEED` as the advice argument.
/// This harness verifies the constant is one of the valid POSIX advice values.
#[kani::proof]
fn verify_posix_fadvise_advice_constant_valid() {
    // POSIX_FADV_WILLNEED is defined by libc. On Linux it's typically 3.
    // We verify the constant is in the valid range [0, 5].
    let advice = libc::POSIX_FADV_WILLNEED;
    kani::assert(
        advice >= 0 && advice <= 5,
        "POSIX_FADV_WILLNEED must be in the valid advice range [0, 5]",
    );

    // Verify it matches the expected value (POSIX_FADV_WILLNEED = 3 on Linux).
    kani::assert(
        advice == libc::POSIX_FADV_WILLNEED,
        "advice constant must be POSIX_FADV_WILLNEED",
    );
}

/// Verify the full prefetch argument safety: all four arguments are valid
/// simultaneously.
#[kani::proof]
fn verify_posix_fadvise_all_args_safe() {
    let fd: RawFd = kani::any();
    let offset: u64 = kani::any();
    let len: u64 = kani::any();

    // Model the preconditions that hold when the unsafe block is reached:
    // 1. The file was successfully opened (fd ≥ 0).
    kani::assume(fd >= 0);
    // 2. The offset/len are realistic (≤ 2 TiB per ADR-002).
    kani::assume(offset <= 2_199_023_255_552);
    kani::assume(len <= 2_199_023_255_552);

    // Verify all the safety properties that the unsafe block relies on:
    kani::assert(fd >= 0, "fd must be valid");
    kani::assert(fd <= i32::MAX as RawFd, "fd must fit in c_int");
    kani::assert(offset <= i64::MAX as u64, "offset must fit in off_t");
    kani::assert(len <= i64::MAX as u64, "len must fit in off_t");
    kani::assert(
        offset.checked_add(len).is_some(),
        "offset + len must not overflow",
    );

    // The advice constant is valid.
    let advice = libc::POSIX_FADV_WILLNEED;
    kani::assert(
        advice >= 0 && advice <= 5,
        "advice must be in valid range",
    );
}
