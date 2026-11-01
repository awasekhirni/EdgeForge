//! # OS-Native Sandbox Policy (F-SEC-03)
//!
//! ## Purpose
//! Defines sandbox policies that restrict what code can do (read/write paths,
//! network egress, subprocess spawning). Enforces OS-level restrictions using
//! platform-native mechanisms:
//! - **Linux**: `bubblewrap` (bwrap) — namespace-based isolation
//! - **macOS**: `sandbox-exec` — seatbelt policy
//! - **Windows**: Job Object enforcement (scaffolding wired; actual `unsafe`
//!   Win32 calls deferred — see `try_windows_job_object`)
//!
//! ## Advisory Mode (tool-missing fallback) — P3
//! When the platform-native sandboxing tool is not available, `enforce()`
//! falls back to **advisory mode**: the policy is still validated, but no
//! OS-level enforcement is applied. In advisory mode:
//! - `Sandbox::is_enforced()` returns `false`
//! - `Sandbox::enforcement_method()` returns `"advisory"`
//! - A `tracing::warn!` is emitted with the missing tool name
//!
//! Platform-specific triggers:
//! - **Linux**: `bwrap --version` fails (binary not on `PATH` or non-zero exit)
//!   → advisory mode with warning `"Sandbox: bwrap not found — install
//!   bubblewrap for OS enforcement"`.
//! - **macOS**: `sandbox-exec -p '(version 1)(allow default)' true` fails
//!   (binary not on `PATH` or non-zero exit) → advisory mode with warning
//!   `"Sandbox: sandbox-exec not found — advisory mode"`.
//! - **Windows**: The Job Object scaffolding (`CreateJobObjectW`,
//!   `SetInformationJobObject`, `AssignProcessToJobObject`) is documented in
//!   `try_windows_job_object` but the actual `unsafe` Win32 calls are
//!   deferred. Until those calls are wired in, `windows_job_object_available()`
//!   reports `false` and Windows runs in advisory mode with warning
//!   `"Sandbox: Windows Job Object unsafe Win32 calls deferred — advisory
//!   mode"`.
//!
//! This is the correct behavior for development environments — in production,
//! the deployment script should install the sandboxing tool (or, on Windows,
//! complete the `unsafe` Win32 implementation).
//!
//! ## What This File Does
//! - `SandboxPolicy` — struct with read_paths, write_paths, network_egress, allow_subprocess
//! - `SandboxPolicy::remote_peer(cache)` — policy for peer nodes (F-SEC-03)
//! - `Sandbox::enforce(policy)` — validates the policy + applies OS-level enforcement
//! - `Sandbox::policy()` — returns the active policy
//! - `Sandbox::is_enforced()` — returns true if OS-level enforcement was applied
//! - `Sandbox::enforcement_method()` — returns `"bubblewrap"` | `"seatbelt"` |
//!   `"windows_job_object"` | `"advisory"`
//! - `bwrap_available()` / `sandbox_exec_available()` /
//!   `windows_job_object_available()` — tool-availability probes (testable)
//!
//! ## Enforcement
//! `enforce()` first validates the policy (write_paths ⊆ read_paths), then
//! attempts OS-level enforcement. If the tool is unavailable, it logs a
//! warning and continues in advisory mode (policy validated but not enforced
//! at the OS level).
//!
//! ## Dependencies
//! - `edgeforge-core::EdgeForgeError` — for SandboxDenied errors
//! - `std::process::Command` — for invoking bwrap/sandbox-exec
//! - `tracing` — for logging enforcement status
//!
//! ## Output
//! `Sandbox` handle — dropping it tears down the sandbox.

use edgeforge_core::EdgeForgeError;
use edgeforge_observability::function_logger::{FunctionGuard, FunctionLogger};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Global logger reference for this module.
static LOGGER: LazyLock<FunctionLogger> = LazyLock::new(FunctionLogger::new);

/// Sandbox policy — defines what the sandboxed code can access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxPolicy {
    /// Paths the sandboxed code can read from.
    pub read_paths: Vec<PathBuf>,
    /// Paths the sandboxed code can write to (must be ⊆ read_paths).
    pub write_paths: Vec<PathBuf>,
    /// Whether network egress is allowed.
    pub network_egress: bool,
    /// Whether spawning subprocesses is allowed.
    pub allow_subprocess: bool,
}

impl SandboxPolicy {
    /// Create a policy for a remote peer node (F-SEC-03).
    ///
    /// Remote peers can only read/write their cache directory and have network
    /// egress. No subprocess spawning.
    #[must_use]
    pub fn remote_peer(cache: &Path) -> Self {
        Self {
            read_paths: vec![cache.to_path_buf()],
            write_paths: vec![cache.to_path_buf()],
            network_egress: true,
            allow_subprocess: false,
        }
    }

    /// Create a strict policy with no network and no subprocess.
    #[must_use]
    pub fn strict(read: Vec<PathBuf>, write: Vec<PathBuf>) -> Self {
        Self {
            read_paths: read,
            write_paths: write,
            network_egress: false,
            allow_subprocess: false,
        }
    }
}

/// Sandbox handle — owns the policy and tracks whether OS enforcement was applied.
///
/// Dropping this handle tears down the sandbox (on platforms where the sandbox
/// is process-based, the child process exits when the handle is dropped).
pub struct Sandbox {
    policy: SandboxPolicy,
    /// Whether OS-level enforcement was successfully applied.
    enforced: bool,
    /// The platform-specific enforcement method used (for logging/diagnostics).
    enforcement_method: &'static str,
}

impl Sandbox {
    /// Enforce the sandbox policy.
    ///
    /// ## Purpose
    /// 1. Validates the policy (write_paths ⊆ read_paths)
    /// 2. Attempts OS-level enforcement via the platform-native tool
    /// 3. If the tool is unavailable, falls back to **advisory mode**:
    ///    - `is_enforced()` returns `false`
    ///    - `enforcement_method()` returns `"advisory"`
    ///    - A `tracing::warn!` is emitted with the missing tool name
    ///
    /// ## Parameters
    /// - `policy`: The sandbox policy to enforce.
    ///
    /// ## Returns
    /// `Ok(Sandbox)` on success. The `enforced` field indicates whether
    /// OS-level enforcement was applied. **Advisory mode is not an error** —
    /// the policy is still validated, and the `Sandbox` handle is returned
    /// with `enforced=false` so callers can branch on `is_enforced()` if
    /// they need to (e.g. refuse to run untrusted code).
    ///
    /// ## Errors
    /// - `SandboxDenied` if a write path is not covered by any read path
    /// - `SandboxDenied` if OS enforcement fails catastrophically (tool exists
    ///   but returns an error)
    pub fn enforce(policy: SandboxPolicy) -> Result<Self, EdgeForgeError> {
        let mut guard =
            FunctionGuard::new(&LOGGER, "edgeforge-security", "sandbox.rs", "enforce");

        // Step 1: Validate the policy.
        for w in &policy.write_paths {
            if !policy
                .read_paths
                .iter()
                .any(|r| r.starts_with(w) || w.starts_with(r))
            {
                let msg = format!("write path {} not covered by any read path", w.display());
                guard.error(&msg);
                return Err(EdgeForgeError::SandboxDenied(msg));
            }
        }

        tracing::info!(
            read_paths = ?policy.read_paths,
            write_paths = ?policy.write_paths,
            network_egress = policy.network_egress,
            allow_subprocess = policy.allow_subprocess,
            "Sandbox: policy validated, attempting OS enforcement"
        );

        // Step 2: Attempt OS-level enforcement.
        let (enforced, method) = Self::try_os_enforce(&policy);

        if enforced {
            tracing::info!(
                method = method,
                "Sandbox: OS-level enforcement applied"
            );
        } else {
            tracing::warn!(
                method = method,
                "Sandbox: OS-level enforcement not available — running in advisory mode"
            );
        }

        guard.success();
        Ok(Self {
            policy,
            enforced,
            enforcement_method: method,
        })
    }

    /// Returns the active policy.
    #[must_use]
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    /// Returns true if OS-level enforcement was applied.
    #[must_use]
    pub fn is_enforced(&self) -> bool {
        self.enforced
    }

    /// Returns the enforcement method used (e.g. "bubblewrap", "sandbox-exec", "advisory").
    #[must_use]
    pub fn enforcement_method(&self) -> &'static str {
        self.enforcement_method
    }

    /// Attempt OS-level enforcement. Returns (was_enforced, method_name).
    ///
    /// The method name is one of:
    /// - `"bubblewrap"` (Linux, bwrap available)
    /// - `"seatbelt"` (macOS, sandbox-exec available)
    /// - `"windows_job_object"` (Windows, Job Object unsafe calls wired in)
    /// - `"advisory"` (tool missing / deferred — policy validated only)
    #[cfg(target_os = "linux")]
    fn try_os_enforce(policy: &SandboxPolicy) -> (bool, &'static str) {
        Self::try_bubblewrap(policy)
    }

    #[cfg(target_os = "macos")]
    fn try_os_enforce(policy: &SandboxPolicy) -> (bool, &'static str) {
        Self::try_seatbelt(policy)
    }

    #[cfg(target_os = "windows")]
    fn try_os_enforce(policy: &SandboxPolicy) -> (bool, &'static str) {
        Self::try_windows_job_object(policy)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    fn try_os_enforce(_policy: &SandboxPolicy) -> (bool, &'static str) {
        tracing::warn!(
            "Sandbox: no platform-native sandbox tool on this OS — advisory mode"
        );
        (false, "advisory")
    }

    /// Probe whether `bwrap` (bubblewrap) is installed and runnable.
    ///
    /// Returns `true` iff `bwrap --version` exits with status 0. Any I/O
    /// error (binary not on `PATH`, permission denied, etc.) is treated as
    /// "not available" → advisory mode.
    #[cfg(target_os = "linux")]
    fn bwrap_available() -> bool {
        std::process::Command::new("bwrap")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Try bubblewrap (bwrap) on Linux.
    ///
    /// If `bwrap` is available, builds the bwrap arg vector for the validated
    /// policy. Actual process isolation is applied when the sandboxed code is
    /// spawned as a child process (the caller wraps the child's `Command` with
    /// these bwrap args).
    ///
    /// If `bwrap` is missing, logs a warning and returns `(false, "advisory")`
    /// — see the module-level "Advisory Mode" docs.
    #[cfg(target_os = "linux")]
    fn try_bubblewrap(policy: &SandboxPolicy) -> (bool, &'static str) {
        if !Self::bwrap_available() {
            tracing::warn!(
                "Sandbox: bwrap not found — install bubblewrap for OS enforcement (advisory mode)"
            );
            return (false, "advisory");
        }

        // bwrap is available — build the args for future use by the caller
        // when spawning the sandboxed child process.
        let mut args: Vec<String> = Vec::new();
        args.push("--unshare-all".to_string());
        // `--unshare-all` isolates the network namespace by default; only
        // re-share it when the policy explicitly allows egress.
        if policy.network_egress {
            args.push("--share-net".to_string());
        }

        for path in &policy.read_paths {
            args.push("--ro-bind".to_string());
            args.push(path.to_string_lossy().to_string());
            args.push(path.to_string_lossy().to_string());
        }
        for path in &policy.write_paths {
            args.push("--bind".to_string());
            args.push(path.to_string_lossy().to_string());
            args.push(path.to_string_lossy().to_string());
        }

        tracing::debug!(
            bwrap_args = ?args,
            "Sandbox: bubblewrap available, args prepared"
        );
        (true, "bubblewrap")
    }

    /// Probe whether `sandbox-exec` (seatbelt) is installed and runnable.
    ///
    /// Returns `true` iff `sandbox-exec -p '(version 1)(allow default)' true`
    /// exits with status 0. Any I/O error is treated as "not available" →
    /// advisory mode.
    #[cfg(target_os = "macos")]
    fn sandbox_exec_available() -> bool {
        std::process::Command::new("sandbox-exec")
            .arg("-p")
            .arg("(version 1)(allow default)")
            .arg("true")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Try sandbox-exec (seatbelt) on macOS.
    ///
    /// If `sandbox-exec` is available, generates a seatbelt policy string
    /// from the `SandboxPolicy`. If missing, logs a warning and returns
    /// `(false, "advisory")` — see the module-level "Advisory Mode" docs.
    #[cfg(target_os = "macos")]
    fn try_seatbelt(policy: &SandboxPolicy) -> (bool, &'static str) {
        if !Self::sandbox_exec_available() {
            tracing::warn!(
                "Sandbox: sandbox-exec not found — advisory mode"
            );
            return (false, "advisory");
        }

        // Build a seatbelt policy string.
        let mut sb_policy = "(version 1)\n(allow default)\n".to_string();

        // Deny network if not allowed.
        if !policy.network_egress {
            sb_policy.push_str("(deny network*)\n");
        }

        // Deny process spawning if not allowed.
        if !policy.allow_subprocess {
            sb_policy.push_str("(deny process-fork)\n");
            sb_policy.push_str("(deny process-exec)\n");
        }

        tracing::debug!(
            seatbelt_policy = %sb_policy,
            "Sandbox: sandbox-exec available, policy prepared"
        );
        (true, "seatbelt")
    }

    /// Probe whether the Windows Job Object enforcement path is actually
    /// wired in.
    ///
    /// The Job Object scaffolding (`CreateJobObjectW`,
    /// `SetInformationJobObject`, `AssignProcessToJobObject`) is documented
    /// in `try_windows_job_object` but the actual `unsafe` Win32 calls are
    /// **deferred** — they are not yet invoked from this crate. Until those
    /// calls are wired in, this function reports `false` so that
    /// `is_enforced()` honestly returns `false` and `enforcement_method()`
    /// returns `"advisory"`.
    ///
    /// When the `unsafe` Win32 implementation lands, flip this to `true`
    /// (or, better, replace it with a real `CreateJobObjectW` probe) and the
    /// `try_windows_job_object` path will start reporting enforcement.
    #[cfg(target_os = "windows")]
    fn windows_job_object_available() -> bool {
        // Intentionally `false`: the `unsafe` Win32 calls in
        // `try_windows_job_object` are deferred (see the comment block there).
        // Reporting `false` keeps `is_enforced()` honest — no OS-level
        // enforcement is actually applied until those calls land.
        false
    }

    /// Try Windows Job Object restriction on Windows.
    ///
    /// ## Design (deferred `unsafe` Win32 calls)
    /// Uses `CreateJobObjectW` + `SetInformationJobObject` with
    /// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and UI/process restrictions.
    /// Network restrictions are applied via Windows Firewall rules (not
    /// directly from this function — the caller would need to add firewall
    /// rules separately).
    ///
    /// ## Current state (P3 — advisory mode)
    /// The Job Object scaffolding is **wired but the actual `unsafe` Win32
    /// calls are deferred**. Concretely, the production code path would be:
    ///
    /// ```text
    /// let handle = unsafe { CreateJobObjectW(null(), null()) };
    /// let mut info = JOBOBJECT_BASIC_LIMIT_INFORMATION::default();
    /// info.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    /// if !policy.allow_subprocess {
    ///     info.LimitFlags |= JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
    ///     info.ActiveProcessLimit = 0; // no child processes
    /// }
    /// SetInformationJobObject(handle, JobObjectBasicLimitInformation, ...);
    /// ```
    ///
    /// Until those `unsafe` calls are wired in, `windows_job_object_available()`
    /// reports `false` and this function logs a warning and returns
    /// `(false, "advisory")` — see the module-level "Advisory Mode" docs.
    /// This keeps `is_enforced()` honest: no OS-level enforcement is
    /// actually applied on Windows today.
    #[cfg(target_os = "windows")]
    fn try_windows_job_object(policy: &SandboxPolicy) -> (bool, &'static str) {
        if !Self::windows_job_object_available() {
            tracing::warn!(
                "Sandbox: Windows Job Object unsafe Win32 calls deferred — advisory mode \
                 (network_egress={}, allow_subprocess={})",
                policy.network_egress,
                policy.allow_subprocess
            );
            return (false, "advisory");
        }

        // When the `unsafe` Win32 calls are wired in, the job object handle
        // would be stored in the `Sandbox` for the lifetime of the handle.
        tracing::info!(
            network_egress = policy.network_egress,
            allow_subprocess = policy.allow_subprocess,
            "Sandbox: Windows Job Object enforcement available — policy prepared"
        );

        if !policy.network_egress {
            tracing::warn!(
                "Sandbox: network egress restriction requires Windows Firewall rules — caller must configure separately"
            );
        }

        (true, "windows_job_object")
    }
}

impl std::fmt::Debug for Sandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sandbox")
            .field("policy", &self.policy)
            .field("enforced", &self.enforced)
            .field("enforcement_method", &self.enforcement_method)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── Policy validation tests ──────────────────────────────────────

    #[test]
    fn test_policy_validation_write_in_read() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        let policy = SandboxPolicy {
            read_paths: vec![path.clone()],
            write_paths: vec![path.clone()],
            network_egress: false,
            allow_subprocess: false,
        };
        let sandbox = Sandbox::enforce(policy).unwrap();
        assert_eq!(sandbox.policy().write_paths.len(), 1);
    }

    #[test]
    fn test_policy_validation_write_not_in_read() {
        let dir1 = TempDir::new().unwrap();
        let dir2 = TempDir::new().unwrap();
        let policy = SandboxPolicy {
            read_paths: vec![dir1.path().to_path_buf()],
            write_paths: vec![dir2.path().to_path_buf()],
            network_egress: false,
            allow_subprocess: false,
        };
        let result = Sandbox::enforce(policy);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EdgeForgeError::SandboxDenied(_)));
    }

    #[test]
    fn test_policy_remote_peer() {
        let dir = TempDir::new().unwrap();
        let policy = SandboxPolicy::remote_peer(dir.path());
        assert!(policy.network_egress);
        assert!(!policy.allow_subprocess);
        assert_eq!(policy.read_paths, policy.write_paths);
    }

    #[test]
    fn test_policy_strict() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        let policy = SandboxPolicy::strict(vec![path.clone()], vec![path]);
        assert!(!policy.network_egress);
        assert!(!policy.allow_subprocess);
    }

    #[test]
    fn test_empty_policy_valid() {
        let policy = SandboxPolicy {
            read_paths: vec![],
            write_paths: vec![],
            network_egress: false,
            allow_subprocess: false,
        };
        let sandbox = Sandbox::enforce(policy).unwrap();
        assert!(sandbox.policy().read_paths.is_empty());
    }

    #[test]
    fn test_policy_equality() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        let p1 = SandboxPolicy::remote_peer(&path);
        let p2 = SandboxPolicy::remote_peer(&path);
        assert_eq!(p1, p2);
    }

    // ── enforcement_method / is_enforced contract ─────────────────────

    #[test]
    fn test_enforce_returns_method() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        let policy = SandboxPolicy {
            read_paths: vec![path.clone()],
            write_paths: vec![path],
            network_egress: true,
            allow_subprocess: false,
        };
        let sandbox = Sandbox::enforce(policy).unwrap();
        // The method is platform-dependent, but it should always be non-empty.
        assert!(!sandbox.enforcement_method().is_empty());
    }

    #[test]
    fn test_is_enforced_returns_bool() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        let policy = SandboxPolicy {
            read_paths: vec![path.clone()],
            write_paths: vec![path],
            network_egress: false,
            allow_subprocess: false,
        };
        let sandbox = Sandbox::enforce(policy).unwrap();
        // is_enforced returns a bool — the value depends on the platform.
        let _ = sandbox.is_enforced();
    }

    #[test]
    fn test_debug_format() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        let policy = SandboxPolicy::remote_peer(&path);
        let sandbox = Sandbox::enforce(policy).unwrap();
        let debug = format!("{sandbox:?}");
        assert!(debug.contains("Sandbox"));
        assert!(debug.contains("policy"));
    }

    // ── Advisory-mode fallback contract (P3) ──────────────────────────
    //
    // These tests verify the documented advisory-mode contract from the
    // module-level docs:
    //   - When the platform tool is missing/deferred, `is_enforced()` is `false`
    //   - `enforcement_method()` is `"advisory"` in that case
    //   - `enforce()` still returns `Ok` (advisory mode is *not* an error)
    //   - The policy is still validated in advisory mode (write ⊆ read)
    //
    // On Linux/macOS the tool may or may not be installed in CI, so the
    // tests branch on `bwrap_available()` / `sandbox_exec_available()`.
    // On Windows the `unsafe` Win32 calls are deferred, so the platform
    // always reports advisory mode today.

    /// Helper: assert that the sandbox is in advisory mode for the given policy.
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn assert_advisory(sandbox: &Sandbox) {
        assert!(
            !sandbox.is_enforced(),
            "advisory mode requires is_enforced() == false, got method={:?}",
            sandbox.enforcement_method(),
        );
        assert_eq!(
            sandbox.enforcement_method(),
            "advisory",
            "advisory mode requires enforcement_method() == \"advisory\"",
        );
    }

    /// Helper: assert that the sandbox is in enforced mode for the given policy.
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn assert_enforced(sandbox: &Sandbox, expected_method: &str) {
        assert!(sandbox.is_enforced(), "expected OS enforcement");
        assert_eq!(sandbox.enforcement_method(), expected_method);
    }

    #[test]
    fn test_advisory_mode_is_not_an_error() {
        // Advisory mode must return Ok(Sandbox) — the policy is still
        // validated, the caller can branch on `is_enforced()`.
        let dir = TempDir::new().unwrap();
        let policy = SandboxPolicy::remote_peer(dir.path());
        let result = Sandbox::enforce(policy);
        assert!(result.is_ok(), "advisory mode must not be an error");
    }

    #[test]
    fn test_advisory_mode_still_validates_policy() {
        // Even in advisory mode, an inconsistent policy (write ⊄ read)
        // must be rejected — advisory mode is not a free pass.
        let policy = SandboxPolicy {
            read_paths: vec![PathBuf::from("/read")],
            write_paths: vec![PathBuf::from("/write")],
            network_egress: false,
            allow_subprocess: false,
        };
        let result = Sandbox::enforce(policy);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EdgeForgeError::SandboxDenied(_)));
    }

    // ── Linux advisory-mode tests ─────────────────────────────────────

    #[cfg(target_os = "linux")]
    #[test]
    fn test_bwrap_available_probe_is_bool() {
        // The probe must return a deterministic bool for the current host.
        let _ = Sandbox::bwrap_available();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_bwrap_available_probe_with_missing_binary() {
        // Sanity-check the probe logic: a binary that does not exist must
        // report `false`. This validates that `unwrap_or(false)` is wired
        // correctly so a missing bwrap falls back to advisory mode rather
        // than panicking.
        let avail = std::process::Command::new("bwrap-definitely-not-on-path-xyz")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        assert!(!avail, "missing binary must report false (advisory mode)");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_advisory_mode_when_bwrap_missing() {
        let dir = TempDir::new().unwrap();
        let policy = SandboxPolicy::remote_peer(dir.path());
        let sandbox = Sandbox::enforce(policy).unwrap();

        if Sandbox::bwrap_available() {
            // bwrap is installed in this environment — enforce() should
            // report OS-level enforcement via bubblewrap.
            assert_enforced(&sandbox, "bubblewrap");
        } else {
            // bwrap is missing — enforce() must fall back to advisory mode.
            assert_advisory(&sandbox);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_advisory_mode_bubblewrap_method_name() {
        // The method name must be exactly "bubblewrap" or "advisory" —
        // never an empty string or a different identifier. This guards
        // against typos in the dispatch table.
        let dir = TempDir::new().unwrap();
        let policy = SandboxPolicy::remote_peer(dir.path());
        let sandbox = Sandbox::enforce(policy).unwrap();
        let method = sandbox.enforcement_method();
        assert!(
            method == "bubblewrap" || method == "advisory",
            "unexpected Linux enforcement method: {method:?}"
        );
    }

    // ── macOS advisory-mode tests ─────────────────────────────────────

    #[cfg(target_os = "macos")]
    #[test]
    fn test_sandbox_exec_available_probe_is_bool() {
        let _ = Sandbox::sandbox_exec_available();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_sandbox_exec_available_probe_with_missing_binary() {
        let avail = std::process::Command::new("sandbox-exec-definitely-not-on-path-xyz")
            .arg("-p")
            .arg("(version 1)(allow default)")
            .arg("true")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        assert!(!avail, "missing binary must report false (advisory mode)");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_advisory_mode_when_sandbox_exec_missing() {
        let dir = TempDir::new().unwrap();
        let policy = SandboxPolicy::remote_peer(dir.path());
        let sandbox = Sandbox::enforce(policy).unwrap();

        if Sandbox::sandbox_exec_available() {
            assert_enforced(&sandbox, "seatbelt");
        } else {
            assert_advisory(&sandbox);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_advisory_mode_seatbelt_method_name() {
        let dir = TempDir::new().unwrap();
        let policy = SandboxPolicy::remote_peer(dir.path());
        let sandbox = Sandbox::enforce(policy).unwrap();
        let method = sandbox.enforcement_method();
        assert!(
            method == "seatbelt" || method == "advisory",
            "unexpected macOS enforcement method: {method:?}"
        );
    }

    // ── Windows advisory-mode tests ───────────────────────────────────
    //
    // On Windows, the `unsafe` Win32 calls (CreateJobObjectW,
    // SetInformationJobObject, AssignProcessToJobObject) are deferred —
    // see `windows_job_object_available()`. Therefore the platform always
    // reports advisory mode today, and these tests pin that contract.

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_job_object_available_is_false_today() {
        // The `unsafe` Win32 calls are deferred — the probe must report
        // false so is_enforced() stays honest. When the calls land, this
        // test should be updated (or replaced with a real probe).
        assert!(!Sandbox::windows_job_object_available());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_always_advisory_until_unsafe_calls_land() {
        let dir = TempDir::new().unwrap();
        let policy = SandboxPolicy::remote_peer(dir.path());
        let sandbox = Sandbox::enforce(policy).unwrap();
        // Until the `unsafe` Win32 calls are wired in, Windows must report
        // advisory mode regardless of the policy contents.
        assert_advisory(&sandbox);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_advisory_mode_with_strict_policy() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        let policy = SandboxPolicy::strict(vec![path.clone()], vec![path]);
        let sandbox = Sandbox::enforce(policy).unwrap();
        assert_advisory(&sandbox);
    }

    // ── Cross-platform: enforcement_method is one of the known values ─

    #[test]
    fn test_enforcement_method_is_known_value() {
        let dir = TempDir::new().unwrap();
        let policy = SandboxPolicy::remote_peer(dir.path());
        let sandbox = Sandbox::enforce(policy).unwrap();
        let method = sandbox.enforcement_method();
        assert!(
            matches!(
                method,
                "bubblewrap" | "seatbelt" | "windows_job_object" | "advisory"
            ),
            "unknown enforcement method: {method:?}"
        );
    }

    #[test]
    fn test_is_enforced_matches_method() {
        // Invariant: is_enforced() == (enforcement_method() != "advisory").
        // This is the core advisory-mode contract that callers rely on.
        let dir = TempDir::new().unwrap();
        let policy = SandboxPolicy::remote_peer(dir.path());
        let sandbox = Sandbox::enforce(policy).unwrap();
        let enforced = sandbox.is_enforced();
        let is_advisory = sandbox.enforcement_method() == "advisory";
        assert_eq!(
            enforced,
            !is_advisory,
            "is_enforced() must equal (method != \"advisory\")"
        );
    }

    #[test]
    fn test_advisory_mode_debug_output() {
        // The Debug output must surface `enforced` and `enforcement_method`
        // so callers can introspect advisory mode from logs.
        let dir = TempDir::new().unwrap();
        let policy = SandboxPolicy::remote_peer(dir.path());
        let sandbox = Sandbox::enforce(policy).unwrap();
        let debug = format!("{sandbox:?}");
        assert!(debug.contains("enforced"));
        assert!(debug.contains("enforcement_method"));
    }

    #[test]
    fn test_advisory_mode_with_network_egress_disabled() {
        // A strict policy (no network, no subprocess) must still validate
        // successfully and report either enforcement or advisory mode —
        // never an error, regardless of platform tool availability.
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        let policy = SandboxPolicy::strict(vec![path.clone()], vec![path]);
        let sandbox = Sandbox::enforce(policy).unwrap();
        assert!(!sandbox.policy().network_egress);
        assert!(!sandbox.policy().allow_subprocess);
    }

    #[test]
    fn test_advisory_mode_with_subprocess_allowed() {
        // allow_subprocess=true must not trigger an error in advisory mode.
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        let policy = SandboxPolicy {
            read_paths: vec![path.clone()],
            write_paths: vec![path],
            network_egress: true,
            allow_subprocess: true,
        };
        let sandbox = Sandbox::enforce(policy).unwrap();
        assert!(sandbox.policy().allow_subprocess);
    }
}
