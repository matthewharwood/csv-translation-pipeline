//! Error types for the translation pipeline.
//!
//! This module defines a unified error enum for all translation operations.
//! Each variant provides actionable context to help diagnose and fix issues.
//!
//! # Design Rationale
//!
//! We use an enum rather than trait objects (`Box<dyn Error>`) because:
//!
//! 1. **Pattern Matching**: Callers can match on specific variants to handle
//!    different failure modes (e.g., retry on network errors, fail fast on
//!    configuration errors).
//!
//! 2. **Zero Heap Allocation**: Error paths avoid allocation overhead.
//!
//! 3. **Exhaustiveness Checking**: The compiler ensures all cases are handled.
//!
//! # Example: Handling Specific Errors
//!
//! ```rust
//! use translator::TranslateError;
//!
//! fn handle_error(err: TranslateError) {
//!     match &err {
//!         TranslateError::Io(e) => {
//!             eprintln!("File error: {e}");
//!             // Suggestion: Check file path and permissions
//!         }
//!         TranslateError::ColumnNotFound { column, available } => {
//!             eprintln!("Column '{column}' not found. Available: {available:?}");
//!             // Suggestion: Use one of the available column names
//!         }
//!         TranslateError::Http(e) if e.is_timeout() => {
//!             eprintln!("Request timed out - consider increasing timeout");
//!         }
//!         _ => eprintln!("Error: {err}"),
//!     }
//! }
//! ```

use thiserror::Error;

/// Errors that can occur during translation operations.
///
/// Each variant includes context to help diagnose the issue and suggests
/// how to resolve it when appropriate.
#[derive(Debug, Error)]
pub enum TranslateError {
    /// Failed to read or write files.
    ///
    /// **Common causes:**
    /// - Input file does not exist
    /// - Output directory is not writable
    /// - Insufficient disk space
    ///
    /// **Resolution:** Verify the file path exists and you have appropriate permissions.
    #[error("File operation failed: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP request to translation provider failed.
    ///
    /// **Common causes:**
    /// - Translation server is not running
    /// - Network connectivity issues
    /// - Request timeout (increase with `--timeout-ms`)
    ///
    /// **Resolution:** Verify the translation server URL is correct and the server is running.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// CSV parsing or writing failed.
    ///
    /// **Common causes:**
    /// - Malformed CSV (unbalanced quotes, wrong encoding)
    /// - Empty input file
    ///
    /// **Resolution:** Validate the CSV format. Ensure UTF-8 encoding.
    #[error("CSV processing failed: {0}")]
    Csv(#[from] csv_async::Error),

    /// The specified source column was not found in the CSV headers.
    ///
    /// **Resolution:** Use `--source-col` to specify one of the available columns.
    #[error(
        "Column '{column}' not found in CSV. Available columns: {available:?}. \
        Use --source-col to specify the correct column name."
    )]
    ColumnNotFound {
        /// The column name that was requested but not found.
        column: String,
        /// List of column names that are available in the CSV.
        available: Vec<String>,
    },

    /// Provider returned wrong number of translations.
    ///
    /// This indicates a bug in the provider implementation. The provider
    /// contract requires returning exactly one translation per input text.
    #[error(
        "Translation count mismatch: expected {expected} translations, got {actual}. \
        This is a provider bug - please report it."
    )]
    TranslationCountMismatch {
        /// Number of input texts that were sent for translation.
        expected: usize,
        /// Number of translations that were returned.
        actual: usize,
    },

    /// Unsupported language code.
    ///
    /// **Resolution:** Check the provider documentation for supported language codes.
    /// NLLB uses codes like "eng_Latn", "fra_Latn". Marian uses the same format.
    #[error(
        "Unsupported language code: '{code}'. \
        Supported codes: {supported:?}"
    )]
    UnsupportedLanguage {
        /// The language code that was not recognized.
        code: String,
        /// List of language codes that are supported by this provider.
        supported: Vec<String>,
    },

    /// Translation provider returned an error.
    ///
    /// This is a catch-all for provider-specific issues that don't fit
    /// other categories, such as:
    /// - Rate limiting (after retries exhausted)
    /// - Invalid API response format
    /// - Provider-specific configuration errors
    #[error("Provider error: {0}")]
    Provider(String),

    /// All retry attempts were exhausted.
    ///
    /// **Common causes:**
    /// - Translation server is overloaded
    /// - Persistent network issues
    /// - Server returning 5xx errors
    ///
    /// **Resolution:** Wait and retry later, or check server health.
    #[error(
        "All {attempts} retry attempts exhausted. Last error: {last_error}. \
        Consider increasing --timeout-ms or checking server health."
    )]
    RetriesExhausted {
        /// Number of retry attempts that were made.
        attempts: u32,
        /// The error message from the last attempt.
        last_error: String,
    },
}

impl TranslateError {
    /// Create a ColumnNotFound error with context.
    pub fn column_not_found(column: impl Into<String>, available: Vec<String>) -> Self {
        Self::ColumnNotFound {
            column: column.into(),
            available,
        }
    }

    /// Create a TranslationCountMismatch error.
    pub fn count_mismatch(expected: usize, actual: usize) -> Self {
        Self::TranslationCountMismatch { expected, actual }
    }

    /// Create an UnsupportedLanguage error.
    pub fn unsupported_language(code: impl Into<String>, supported: Vec<String>) -> Self {
        Self::UnsupportedLanguage {
            code: code.into(),
            supported,
        }
    }

    /// Create a RetriesExhausted error.
    pub fn retries_exhausted(attempts: u32, last_error: impl Into<String>) -> Self {
        Self::RetriesExhausted {
            attempts,
            last_error: last_error.into(),
        }
    }

    /// Returns true if this error is potentially transient and worth retrying.
    ///
    /// This is useful for implementing retry logic at higher levels.
    pub fn is_retryable(&self) -> bool {
        match self {
            // Network errors are often transient
            Self::Http(e) => e.is_timeout() || e.is_connect() || e.is_request(),
            // Already exhausted retries - don't retry again
            Self::RetriesExhausted { .. } => false,
            // Configuration errors won't be fixed by retrying
            Self::ColumnNotFound { .. } => false,
            Self::UnsupportedLanguage { .. } => false,
            Self::TranslationCountMismatch { .. } => false,
            // IO errors are usually not transient
            Self::Io(_) => false,
            // CSV errors are not transient
            Self::Csv(_) => false,
            // Provider errors might be transient (could be rate limiting, etc.)
            Self::Provider(_) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_not_found_error_message_is_actionable() {
        let err = TranslateError::column_not_found(
            "wrong_column",
            vec!["source".to_string(), "text".to_string()],
        );

        let msg = err.to_string();

        // Error message should include all useful information
        assert!(msg.contains("wrong_column"), "Should mention the missing column");
        assert!(msg.contains("source"), "Should list available columns");
        assert!(msg.contains("--source-col"), "Should suggest how to fix");
    }

    #[test]
    fn count_mismatch_error_indicates_bug() {
        let err = TranslateError::count_mismatch(10, 5);

        let msg = err.to_string();

        assert!(msg.contains("10"), "Should show expected count");
        assert!(msg.contains("5"), "Should show actual count");
        assert!(msg.contains("provider bug"), "Should indicate this is a bug");
    }

    #[test]
    fn retries_exhausted_suggests_remediation() {
        let err = TranslateError::retries_exhausted(3, "connection refused");

        let msg = err.to_string();

        assert!(msg.contains("3"), "Should show retry count");
        assert!(msg.contains("connection refused"), "Should include last error");
        assert!(msg.contains("timeout"), "Should suggest increasing timeout");
    }

    #[test]
    fn is_retryable_classification() {
        // Configuration errors are not retryable
        assert!(!TranslateError::column_not_found("x", vec![]).is_retryable());
        assert!(!TranslateError::unsupported_language("x", vec![]).is_retryable());
        assert!(!TranslateError::count_mismatch(1, 2).is_retryable());

        // Already retried - don't retry again
        assert!(!TranslateError::retries_exhausted(3, "err").is_retryable());

        // Provider errors might be transient
        assert!(TranslateError::Provider("rate limited".into()).is_retryable());
    }
}
