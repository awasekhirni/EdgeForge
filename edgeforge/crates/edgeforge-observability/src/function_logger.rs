//! # Function Call Logger
//!
//! ## Purpose
//! World-class function-call logging that records every function invocation
//! in the format: `timestamp - crate - file - function - status`
//!
//! ## What This File Does
//! - `FunctionLogger` — thread-safe singleton that accumulates function-call logs
//! - `log_call!` macro — logs function entry with success status
//! - `FunctionGuard` — RAII guard that logs exit (success or error)
//! - `CallLog` — structured log entry (timestamp, crate, file, function, status, duration)
//! - `dump_logs()` — returns all accumulated logs as JSON
//!
//! ## Log Format
//! Each log entry is: `2026-10-31T22:30:00.123Z - edgeforge-net - quic_transport.rs - connect - SUCCESS`
//!
//! ## Usage
//! ```ignore
//! use edgeforge_observability::function_logger::log_call;
//!
//! pub async fn my_function() -> Result<(), Error> {
//!     let _guard = log_call!(); // logs entry, logs exit on drop
//!     // ... function body ...
//!     Ok(())
//! }
//! ```
//!
//! ## Output
//! Structured log entries via `tracing` + in-memory `CallLog` buffer for audit.

use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// A single function-call log entry.
///
/// ## Fields
/// - `timestamp`: ISO 8601 UTC timestamp when the function was called
/// - `crate_name`: Name of the crate containing the function
/// - `file_name`: Name of the source file (without full path)
/// - `function_name`: Name of the function
/// - `status`: "ENTRY", "SUCCESS", or "ERROR"
/// - `duration_us`: Duration in microseconds (0 for ENTRY, actual for SUCCESS/ERROR)
/// - `error_message`: Error message if status is "ERROR", otherwise None
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CallLog {
    /// ISO 8601 UTC timestamp (e.g., "2026-10-31T22:30:00.123Z")
    pub timestamp: String,
    /// Name of the crate (e.g., "edgeforge-net")
    pub crate_name: String,
    /// Source file name without path (e.g., "quic_transport.rs")
    pub file_name: String,
    /// Function name (e.g., "connect")
    pub function_name: String,
    /// Call status: "ENTRY", "SUCCESS", or "ERROR"
    pub status: CallStatus,
    /// Duration in microseconds (0 for ENTRY, actual for SUCCESS/ERROR)
    pub duration_us: u64,
    /// Error message if status is ERROR, otherwise None
    pub error_message: Option<String>,
}

/// The status of a function call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallStatus {
    /// Function was entered.
    Entry,
    /// Function completed successfully.
    Success,
    /// Function returned an error.
    Error,
}

impl std::fmt::Display for CallStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Entry => write!(f, "ENTRY"),
            Self::Success => write!(f, "SUCCESS"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

impl std::fmt::Display for CallLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} - {} - {} - {} - {}",
            self.timestamp, self.crate_name, self.file_name, self.function_name, self.status
        )?;
        if self.duration_us > 0 {
            write!(f, " ({}µs)", self.duration_us)?;
        }
        if let Some(ref msg) = self.error_message {
            write!(f, " [{}]", msg)?;
        }
        Ok(())
    }
}

/// Thread-safe function-call logger. Accumulates all call logs in memory.
///
/// ## Purpose
/// Stores every function call log entry for audit trail and debugging.
/// Uses a bounded buffer (default 100,000 entries) to prevent unbounded memory growth.
///
/// ## Thread Safety
/// Uses `Arc<Mutex<Vec<CallLog>>>` for thread-safe access from any thread.
#[derive(Clone)]
pub struct FunctionLogger {
    logs: Arc<Mutex<Vec<CallLog>>>,
    max_entries: usize,
}

impl FunctionLogger {
    /// Creates a new function logger with the default capacity (100,000 entries).
    ///
    /// ## Returns
    /// A new `FunctionLogger` instance.
    pub fn new() -> Self {
        Self::with_capacity(100_000)
    }

    /// Creates a new function logger with the specified maximum entry count.
    ///
    /// ## Parameters
    /// - `max_entries`: Maximum number of log entries to retain. Older entries
    ///   are dropped when the limit is exceeded.
    ///
    /// ## Returns
    /// A new `FunctionLogger` instance.
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            logs: Arc::new(Mutex::new(Vec::with_capacity(max_entries.min(100_000)))),
            max_entries,
        }
    }

    /// Records a function call log entry.
    ///
    /// ## Parameters
    /// - `crate_name`: Name of the crate
    /// - `file_name`: Source file name
    /// - `function_name`: Function name
    /// - `status`: Call status (ENTRY, SUCCESS, or ERROR)
    /// - `duration_us`: Duration in microseconds
    /// - `error_message`: Optional error message
    ///
    /// ## Side Effects
    /// Appends a `CallLog` to the internal buffer. If the buffer exceeds
    /// `max_entries`, the oldest entry is removed.
    pub fn log(
        &self,
        crate_name: &str,
        file_name: &str,
        function_name: &str,
        status: CallStatus,
        duration_us: u64,
        error_message: Option<String>,
    ) {
        let entry = CallLog {
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            crate_name: crate_name.to_string(),
            file_name: file_name.to_string(),
            function_name: function_name.to_string(),
            status,
            duration_us,
            error_message,
        };

        // Emit to tracing for structured logging.
        match status {
            CallStatus::Entry => {
                tracing::trace!(target: "edgeforge::function_call",
                    timestamp = %entry.timestamp,
                    crate = %entry.crate_name,
                    file = %entry.file_name,
                    function = %entry.function_name,
                    status = %entry.status,
                    "FUNCTION_CALL"
                );
            }
            CallStatus::Success => {
                tracing::debug!(target: "edgeforge::function_call",
                    timestamp = %entry.timestamp,
                    crate = %entry.crate_name,
                    file = %entry.file_name,
                    function = %entry.function_name,
                    status = %entry.status,
                    duration_us = entry.duration_us,
                    "FUNCTION_CALL"
                );
            }
            CallStatus::Error => {
                tracing::warn!(target: "edgeforge::function_call",
                    timestamp = %entry.timestamp,
                    crate = %entry.crate_name,
                    file = %entry.file_name,
                    function = %entry.function_name,
                    status = %entry.status,
                    duration_us = entry.duration_us,
                    error = ?entry.error_message,
                    "FUNCTION_CALL"
                );
            }
        }

        // Also store in memory buffer.
        let mut logs = self.logs.lock();
        if logs.len() >= self.max_entries {
            logs.remove(0); // Drop oldest
        }
        logs.push(entry);
    }

    /// Returns all accumulated log entries.
    ///
    /// ## Returns
    /// `Vec<CallLog>` — all log entries, oldest first.
    pub fn dump_logs(&self) -> Vec<CallLog> {
        self.logs.lock().clone()
    }

    /// Returns log entries as a JSON string.
    ///
    /// ## Returns
    /// `String` — JSON array of all log entries.
    pub fn dump_json(&self) -> String {
        serde_json::to_string_pretty(self.dump_logs().as_slice())
            .unwrap_or_else(|_| "[]".to_string())
    }

    /// Returns the number of log entries currently stored.
    ///
    /// ## Returns
    /// `usize` — the number of entries.
    pub fn len(&self) -> usize {
        self.logs.lock().len()
    }

    /// Returns true if no log entries are stored.
    ///
    /// ## Returns
    /// `bool` — true if empty.
    pub fn is_empty(&self) -> bool {
        self.logs.lock().is_empty()
    }

    /// Clears all log entries.
    pub fn clear(&self) {
        self.logs.lock().clear();
    }

    /// Returns recent log entries (last N).
    ///
    /// ## Parameters
    /// - `n`: Number of recent entries to return.
    ///
    /// ## Returns
    /// `Vec<CallLog>` — the last N entries, oldest first.
    pub fn recent(&self, n: usize) -> Vec<CallLog> {
        let logs = self.logs.lock();
        let start = logs.len().saturating_sub(n);
        logs[start..].to_vec()
    }
}

impl Default for FunctionLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a FunctionLogger suitable for use in statics via LazyLock.
/// Not const-constructible due to Arc, but usable with `LazyLock::new(FunctionLogger::new)`.
pub fn const_new() -> FunctionLogger {
    FunctionLogger::new()
}

/// RAII guard that logs function entry on creation and success/error on drop.
///
/// ## Purpose
/// Automatically logs the exit status of a function. When the guard is dropped:
/// - If `mark_success()` was called → logs SUCCESS
/// - If `mark_error(msg)` was called → logs ERROR with the message
/// - If neither was called → logs SUCCESS (default)
///
/// ## Usage
/// ```ignore
/// use edgeforge_observability::function_logger::{FunctionLogger, FunctionGuard};
///
/// static LOGGER: FunctionLogger = FunctionLogger::new();
///
/// pub fn my_function() -> Result<(), Error> {
///     let guard = FunctionGuard::new(&LOGGER, "edgeforge-net", "file.rs", "my_function");
///     // ... function body ...
///     guard.success(); // optional — defaults to success on drop
///     Ok(())
/// }
/// ```
pub struct FunctionGuard<'a> {
    logger: &'a FunctionLogger,
    crate_name: &'static str,
    file_name: &'static str,
    function_name: &'static str,
    start: Instant,
    status: CallStatus,
    error_message: Option<String>,
}

impl<'a> FunctionGuard<'a> {
    /// Creates a new function guard and logs the ENTRY event.
    ///
    /// ## Parameters
    /// - `logger`: Reference to the FunctionLogger
    /// - `crate_name`: Name of the crate (use `env!("CARGO_PKG_NAME")`)
    /// - `file_name`: Source file name (use `file!()`)
    /// - `function_name`: Function name (use a string literal)
    ///
    /// ## Returns
    /// A new `FunctionGuard` that logs exit status on drop.
    pub fn new(
        logger: &'a FunctionLogger,
        crate_name: &'static str,
        file_name: &'static str,
        function_name: &'static str,
    ) -> Self {
        logger.log(crate_name, file_name, function_name, CallStatus::Entry, 0, None);
        Self {
            logger,
            crate_name,
            file_name,
            function_name,
            start: Instant::now(),
            status: CallStatus::Success,
            error_message: None,
        }
    }

    /// Marks the function as completed successfully.
    pub fn success(&mut self) {
        self.status = CallStatus::Success;
        self.error_message = None;
    }

    /// Marks the function as completed with an error.
    ///
    /// ## Parameters
    /// - `message`: The error message.
    pub fn error(&mut self, message: impl Into<String>) {
        self.status = CallStatus::Error;
        self.error_message = Some(message.into());
    }
}

impl<'a> Drop for FunctionGuard<'a> {
    fn drop(&mut self) {
        let duration_us = self.start.elapsed().as_micros() as u64;
        self.logger.log(
            self.crate_name,
            self.file_name,
            self.function_name,
            self.status,
            duration_us,
            self.error_message.take(),
        );
    }
}

/// Macro to create a function guard with automatic crate/file/function detection.
///
/// ## Usage
/// ```ignore
/// use edgeforge_observability::function_logger::log_call;
///
/// pub fn my_function() -> Result<(), Error> {
///     let _guard = log_call!();
///     // ... function body ...
///     Ok(())
/// }
/// ```
///
/// The macro automatically:
/// - Uses the crate name from `CARGO_PKG_NAME`
/// - Uses the file name from `file!()`
/// - Requires a `LOGGER` constant of type `FunctionLogger` in scope
#[macro_export]
macro_rules! log_call {
    () => {{
        use $crate::function_logger::{FunctionGuard, FunctionLogger};
        static LOGGER: FunctionLogger = FunctionLogger::new();
        FunctionGuard::new(
            &LOGGER,
            env!("CARGO_PKG_NAME"),
            file!(),
            concat!(module_path!().split("::").last().unwrap_or("unknown")),
        )
    }};
    ($logger:expr) => {{
        use $crate::function_logger::FunctionGuard;
        FunctionGuard::new(
            $logger,
            env!("CARGO_PKG_NAME"),
            file!(),
            concat!(module_path!().split("::").last().unwrap_or("unknown")),
        )
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_logger_log_and_dump() {
        let logger = FunctionLogger::with_capacity(100);
        logger.log("test-crate", "test.rs", "test_fn", CallStatus::Entry, 0, None);
        logger.log("test-crate", "test.rs", "test_fn", CallStatus::Success, 42, None);

        let logs = logger.dump_logs();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].status, CallStatus::Entry);
        assert_eq!(logs[1].status, CallStatus::Success);
        assert_eq!(logs[1].duration_us, 42);
    }

    #[test]
    fn test_function_logger_max_entries() {
        let logger = FunctionLogger::with_capacity(3);
        for i in 0..5 {
            logger.log("c", "f.rs", &format!("fn_{i}"), CallStatus::Entry, 0, None);
        }
        let logs = logger.dump_logs();
        assert_eq!(logs.len(), 3); // only last 3
        assert_eq!(logs[0].function_name, "fn_2");
    }

    #[test]
    fn test_function_guard_success() {
        let logger = FunctionLogger::with_capacity(100);
        {
            let _guard = FunctionGuard::new(&logger, "test", "file.rs", "my_fn");
            // guard drops here → logs SUCCESS
        }
        let logs = logger.dump_logs();
        assert_eq!(logs.len(), 2); // ENTRY + SUCCESS
        assert_eq!(logs[0].status, CallStatus::Entry);
        assert_eq!(logs[1].status, CallStatus::Success);
    }

    #[test]
    fn test_function_guard_error() {
        let logger = FunctionLogger::with_capacity(100);
        {
            let mut guard = FunctionGuard::new(&logger, "test", "file.rs", "my_fn");
            guard.error("something went wrong");
            // guard drops here → logs ERROR
        }
        let logs = logger.dump_logs();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[1].status, CallStatus::Error);
        assert_eq!(logs[1].error_message.as_deref(), Some("something went wrong"));
    }

    #[test]
    fn test_call_log_display() {
        let log = CallLog {
            timestamp: "2026-10-31T22:30:00.123Z".to_string(),
            crate_name: "edgeforge-net".to_string(),
            file_name: "quic_transport.rs".to_string(),
            function_name: "connect".to_string(),
            status: CallStatus::Success,
            duration_us: 1500,
            error_message: None,
        };
        let s = format!("{log}");
        assert!(s.contains("edgeforge-net"));
        assert!(s.contains("quic_transport.rs"));
        assert!(s.contains("connect"));
        assert!(s.contains("SUCCESS"));
        assert!(s.contains("1500µs"));
    }

    #[test]
    fn test_dump_json() {
        let logger = FunctionLogger::with_capacity(100);
        logger.log("c", "f.rs", "fn", CallStatus::Entry, 0, None);
        let json = logger.dump_json();
        // Check that the JSON contains the expected fields.
        assert!(json.contains("crate_name"), "JSON should contain crate_name: {json}");
        assert!(json.contains("status"), "JSON should contain status: {json}");
    }

    #[test]
    fn test_recent() {
        let logger = FunctionLogger::with_capacity(100);
        for i in 0..10 {
            logger.log("c", "f.rs", &format!("fn_{i}"), CallStatus::Entry, 0, None);
        }
        let recent = logger.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[2].function_name, "fn_9");
    }

    #[test]
    fn test_clear() {
        let logger = FunctionLogger::with_capacity(100);
        logger.log("c", "f.rs", "fn", CallStatus::Entry, 0, None);
        assert!(!logger.is_empty());
        logger.clear();
        assert!(logger.is_empty());
    }
}
