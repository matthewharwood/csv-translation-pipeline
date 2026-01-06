//! NLLB REST API translation provider.
//!
//! Connects to a local NLLB (No Language Left Behind) server running as a
//! REST API. Includes production-ready retry logic with exponential backoff.
//!
//! # Server Requirements
//!
//! This provider expects an NLLB server with a translation endpoint:
//!
//! ```text
//! POST /translate
//! Content-Type: application/json
//!
//! {
//!     "src_lang": "eng_Latn",
//!     "tgt_lang": "fra_Latn",
//!     "text": ["Hello", "World"]
//! }
//!
//! Response:
//! {
//!     "translations": ["Bonjour", "Monde"]
//! }
//! ```
//!
//! # Example
//!
//! ```no_run
//! use translator::provider::nllb_rest::NllbRestProvider;
//! use translator::TranslationProvider;
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), translator::TranslateError> {
//! let provider = NllbRestProvider::new("http://localhost:8080")
//!     .with_timeout(Duration::from_secs(60))
//!     .with_max_retries(5);
//!
//! let translations = provider
//!     .translate_batch("eng_Latn", "fra_Latn", &["Hello".to_string()])
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Retry Behavior
//!
//! The provider automatically retries on transient failures:
//!
//! | Error Type | Retried? |
//! |------------|----------|
//! | Timeout | Yes |
//! | Connection refused | Yes |
//! | HTTP 408 (Request Timeout) | Yes |
//! | HTTP 429 (Too Many Requests) | Yes |
//! | HTTP 5xx (Server Error) | Yes |
//! | HTTP 4xx (Client Error) | No |
//! | Parse error | No |

use crate::error::TranslateError;
use crate::provider::TranslationProvider;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Default configuration values for the NLLB provider.
mod defaults {
    use std::time::Duration;

    /// Default request timeout.
    pub const TIMEOUT: Duration = Duration::from_secs(30);

    /// Default maximum retry attempts.
    pub const MAX_RETRIES: u32 = 3;

    /// Initial backoff delay before first retry.
    pub const BACKOFF_BASE: Duration = Duration::from_millis(250);

    /// Maximum backoff delay between retries.
    pub const BACKOFF_CAP: Duration = Duration::from_secs(4);
}

/// NLLB REST API client with configurable retry behavior.
///
/// Uses the builder pattern for configuration. All settings have sensible
/// defaults for production use.
///
/// # Example
///
/// ```no_run
/// use translator::provider::nllb_rest::NllbRestProvider;
/// use std::time::Duration;
///
/// // Minimal configuration
/// let provider = NllbRestProvider::new("http://localhost:8080");
///
/// // Full configuration
/// let provider = NllbRestProvider::new("http://localhost:8080")
///     .with_timeout(Duration::from_secs(60))
///     .with_max_retries(5)
///     .with_backoff(Duration::from_millis(500), Duration::from_secs(8));
/// ```
#[derive(Clone)]
pub struct NllbRestProvider {
    client: Client,
    base_url: String,
    timeout: Duration,
    max_retries: u32,
    backoff_base: Duration,
    backoff_cap: Duration,
}

impl NllbRestProvider {
    /// Create a new provider pointing to the given NLLB server URL.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL of the NLLB server (e.g., "http://localhost:8080")
    ///
    /// # Defaults
    ///
    /// - Timeout: 30 seconds
    /// - Max retries: 3
    /// - Backoff: 250ms base, 4s cap
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            timeout: defaults::TIMEOUT,
            max_retries: defaults::MAX_RETRIES,
            backoff_base: defaults::BACKOFF_BASE,
            backoff_cap: defaults::BACKOFF_CAP,
        }
    }

    /// Set the request timeout.
    ///
    /// This is the maximum time to wait for a response from the server.
    /// For large batches, consider increasing this value.
    ///
    /// Default: 30 seconds
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set maximum retry attempts for transient failures.
    ///
    /// After this many failures, the provider gives up and returns an error.
    /// Set to 0 to disable retries entirely.
    ///
    /// Default: 3
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Configure exponential backoff parameters.
    ///
    /// - `base`: Initial delay before first retry
    /// - `cap`: Maximum delay between retries
    ///
    /// The actual delay is: `min(base * 2^attempt + jitter, cap)`
    ///
    /// Default: 250ms base, 4s cap
    pub fn with_backoff(mut self, base: Duration, cap: Duration) -> Self {
        self.backoff_base = base;
        self.backoff_cap = cap;
        self
    }
}

// ---------------------------------------------------------------------------
// API Types
// ---------------------------------------------------------------------------

/// Request body for the NLLB translation endpoint.
#[derive(Serialize)]
struct TranslateRequest<'a> {
    src_lang: &'a str,
    tgt_lang: &'a str,
    text: &'a [String],
}

/// Response body from the NLLB translation endpoint.
#[derive(Deserialize)]
struct TranslateResponse {
    translations: Vec<String>,
}

// ---------------------------------------------------------------------------
// Retry Logic
// ---------------------------------------------------------------------------

/// HTTP status codes that indicate transient failures worth retrying.
fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT         // 408 - Server took too long
            | StatusCode::TOO_MANY_REQUESTS // 429 - Rate limited
            | StatusCode::INTERNAL_SERVER_ERROR // 500 - Server error
            | StatusCode::BAD_GATEWAY       // 502 - Upstream error
            | StatusCode::SERVICE_UNAVAILABLE // 503 - Server overloaded
            | StatusCode::GATEWAY_TIMEOUT   // 504 - Upstream timeout
    )
}

/// Network errors that indicate transient failures worth retrying.
fn is_retryable_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

/// Calculate exponential backoff with jitter.
///
/// Formula: `min(base * 2^attempt + random_jitter, cap)`
///
/// Jitter (0-100ms) prevents thundering herd when multiple clients
/// retry simultaneously after a server outage.
fn calculate_backoff(attempt: u32, base: Duration, cap: Duration) -> Duration {
    // Cap the exponent to prevent overflow (2^10 = 1024)
    let multiplier = 1u64 << attempt.min(10);
    let base_ms = base.as_millis() as u64;
    let delay_ms = base_ms.saturating_mul(multiplier);
    let capped_ms = delay_ms.min(cap.as_millis() as u64);

    // Add random jitter to prevent synchronized retries
    let jitter_ms = fastrand::u64(0..=100);

    Duration::from_millis(capped_ms + jitter_ms)
}

// ---------------------------------------------------------------------------
// Provider Implementation
// ---------------------------------------------------------------------------

impl TranslationProvider for NllbRestProvider {
    async fn translate_batch(
        &self,
        src_lang: &str,
        tgt_lang: &str,
        texts: &[String],
    ) -> Result<Vec<String>, TranslateError> {
        // Handle empty input without making a network request
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let url = format!("{}/translate", self.base_url.trim_end_matches('/'));
        let request_body = TranslateRequest {
            src_lang,
            tgt_lang,
            text: texts,
        };

        let mut last_error = String::new();

        for attempt in 0..=self.max_retries {
            let result = self
                .client
                .post(&url)
                .timeout(self.timeout)
                .json(&request_body)
                .send()
                .await;

            match result {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        let parsed: TranslateResponse = response.json().await?;

                        // Validate response length matches input
                        if parsed.translations.len() != texts.len() {
                            return Err(TranslateError::count_mismatch(
                                texts.len(),
                                parsed.translations.len(),
                            ));
                        }

                        return Ok(parsed.translations);
                    }

                    // Non-success status code
                    let body = response.text().await.unwrap_or_default();

                    if is_retryable_status(status) && attempt < self.max_retries {
                        let backoff =
                            calculate_backoff(attempt, self.backoff_base, self.backoff_cap);
                        eprintln!(
                            "[nllb] HTTP {status} on attempt {}/{}, retrying in {backoff:?}",
                            attempt + 1,
                            self.max_retries + 1
                        );
                        sleep(backoff).await;
                        last_error = format!("HTTP {status}: {body}");
                        continue;
                    }

                    // Non-retryable error or retries exhausted
                    return Err(TranslateError::Provider(format!(
                        "NLLB server returned HTTP {status}: {body}"
                    )));
                }

                Err(err) => {
                    if is_retryable_error(&err) && attempt < self.max_retries {
                        let backoff =
                            calculate_backoff(attempt, self.backoff_base, self.backoff_cap);
                        eprintln!(
                            "[nllb] Network error on attempt {}/{}, retrying in {backoff:?}: {err}",
                            attempt + 1,
                            self.max_retries + 1
                        );
                        sleep(backoff).await;
                        last_error = err.to_string();
                        continue;
                    }

                    // Non-retryable error or retries exhausted
                    return Err(err.into());
                }
            }
        }

        // All retries exhausted
        Err(TranslateError::retries_exhausted(
            self.max_retries,
            last_error,
        ))
    }

    fn name(&self) -> &'static str {
        "nllb-rest"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_exponentially() {
        let base = Duration::from_millis(100);
        let cap = Duration::from_secs(10);

        // Without jitter, we'd expect: 100, 200, 400, 800...
        // With jitter (0-100ms), we expect values in those ranges
        let b0 = calculate_backoff(0, base, cap);
        let b1 = calculate_backoff(1, base, cap);
        let b2 = calculate_backoff(2, base, cap);

        // Allow for jitter variance
        assert!(
            b0.as_millis() >= 100 && b0.as_millis() <= 200,
            "Expected 100-200ms, got {:?}",
            b0
        );
        assert!(
            b1.as_millis() >= 200 && b1.as_millis() <= 300,
            "Expected 200-300ms, got {:?}",
            b1
        );
        assert!(
            b2.as_millis() >= 400 && b2.as_millis() <= 500,
            "Expected 400-500ms, got {:?}",
            b2
        );
    }

    #[test]
    fn backoff_respects_cap() {
        let base = Duration::from_millis(1000);
        let cap = Duration::from_millis(500);

        let b = calculate_backoff(10, base, cap);

        // Should be capped at 500ms + jitter (max 100ms)
        assert!(
            b.as_millis() <= 600,
            "Expected max 600ms, got {:?}",
            b
        );
    }

    #[test]
    fn retryable_statuses_are_correct() {
        // These should be retried
        assert!(is_retryable_status(StatusCode::REQUEST_TIMEOUT));
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable_status(StatusCode::GATEWAY_TIMEOUT));

        // These should NOT be retried (client errors)
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED));
        assert!(!is_retryable_status(StatusCode::FORBIDDEN));
        assert!(!is_retryable_status(StatusCode::NOT_FOUND));
    }

    #[test]
    fn builder_pattern_works() {
        let provider = NllbRestProvider::new("http://localhost:8080")
            .with_timeout(Duration::from_secs(60))
            .with_max_retries(10)
            .with_backoff(Duration::from_millis(500), Duration::from_secs(10));

        assert_eq!(provider.timeout, Duration::from_secs(60));
        assert_eq!(provider.max_retries, 10);
        assert_eq!(provider.backoff_base, Duration::from_millis(500));
        assert_eq!(provider.backoff_cap, Duration::from_secs(10));
    }
}
