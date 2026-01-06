//! Async job-based translation API.
//!
//! This module provides a non-blocking job API for translation operations.
//! Submit jobs and poll for results without blocking the calling thread.
//!
//! # When to Use This Module
//!
//! Use the job API when:
//! - Integrating with web frameworks (Axum, Actix, etc.)
//! - You need to track multiple concurrent translations
//! - Long-running translations shouldn't block your event loop
//!
//! For simple file-to-file translation, use [`crate::translate_csv`] instead.
//!
//! # Example: Web Service Integration
//!
//! ```rust
//! use translator::jobs::{JobManager, TranslationRequest, JobStatus};
//! use translator::provider::mock::MockProvider;
//!
//! # async fn example() {
//! // Create a job manager (share across your app via Arc or Extension)
//! let manager = JobManager::new(MockProvider);
//!
//! // Submit a job (returns immediately)
//! let job_id = manager.submit(TranslationRequest {
//!     texts: vec!["Hello".into(), "Goodbye".into()],
//!     src_lang: "eng_Latn".into(),
//!     targets: vec![("fr".into(), "fra_Latn".into())],
//! });
//!
//! // Poll for status (in a real app, client would poll an HTTP endpoint)
//! match manager.get(&job_id) {
//!     None => println!("Job not found"),
//!     Some(JobStatus::Pending) => println!("Still working..."),
//!     Some(JobStatus::Complete { translations }) => {
//!         println!("Done! French: {:?}", translations.get("fr"));
//!     }
//!     Some(JobStatus::Failed { error }) => println!("Error: {error}"),
//! }
//! # }
//! ```
//!
//! # Thread Safety
//!
//! [`JobManager`] is `Clone + Send + Sync` and safe to share across threads.
//! All internal state is protected by `RwLock`.

use crate::provider::TranslationProvider;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public Types
// ---------------------------------------------------------------------------

/// Unique identifier for a translation job.
///
/// Job IDs are UUID v4 strings, guaranteed unique within a single process.
pub type JobId = String;

/// Request to translate texts into multiple languages.
///
/// This is the input to [`JobManager::submit`].
#[derive(Debug, Clone)]
pub struct TranslationRequest {
    /// Texts to translate (order is preserved in output).
    pub texts: Vec<String>,

    /// Source language code (e.g., "eng_Latn").
    pub src_lang: String,

    /// Target languages as (output_column_name, language_code) pairs.
    ///
    /// Example: `vec![("fr".into(), "fra_Latn".into())]`
    pub targets: Vec<(String, String)>,
}

/// Translations organized by target language.
///
/// Maps column name to translated texts (in same order as input).
pub type TranslationResult = HashMap<String, Vec<String>>;

/// Current status of a translation job.
///
/// Jobs progress through states: `Pending` -> `Complete` or `Failed`.
#[derive(Debug, Clone)]
pub enum JobStatus {
    /// Job is queued or in progress.
    Pending,

    /// Job completed successfully.
    Complete {
        /// Translations organized by target language column name.
        translations: TranslationResult,
    },

    /// Job failed with an error.
    Failed {
        /// Human-readable error description.
        error: String,
    },
}

// ---------------------------------------------------------------------------
// JobManager
// ---------------------------------------------------------------------------

/// Manages async translation jobs.
///
/// Thread-safe and cloneable. Share one instance across your application.
///
/// # Memory Management
///
/// Completed jobs remain in memory until explicitly removed with [`remove`](Self::remove).
/// For long-running services, implement periodic cleanup or TTL-based expiration.
///
/// # Example with Axum
///
/// ```rust,ignore
/// use axum::{Router, Extension};
/// use translator::jobs::JobManager;
/// use translator::provider::nllb_rest::NllbRestProvider;
///
/// let provider = NllbRestProvider::new("http://localhost:8080");
/// let manager = JobManager::new(provider);
///
/// let app = Router::new()
///     .route("/translate", post(submit_handler))
///     .route("/jobs/:id", get(status_handler))
///     .layer(Extension(manager));
/// ```
#[derive(Clone)]
pub struct JobManager<P> {
    provider: Arc<P>,
    jobs: Arc<RwLock<HashMap<JobId, JobStatus>>>,
}

impl<P: TranslationProvider> JobManager<P> {
    /// Create a new job manager with the given translation provider.
    ///
    /// The provider is wrapped in `Arc` for cheap cloning.
    pub fn new(provider: P) -> Self {
        Self {
            provider: Arc::new(provider),
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Submit a translation job.
    ///
    /// Returns immediately with a job ID. The translation runs in the background.
    /// Poll with [`get`](Self::get) to check status.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (indicates a bug).
    pub fn submit(&self, request: TranslationRequest) -> JobId {
        let job_id = Uuid::new_v4().to_string();

        // Mark as pending before spawning to avoid race conditions
        {
            let mut jobs = self.jobs.write().expect("lock not poisoned");
            jobs.insert(job_id.clone(), JobStatus::Pending);
        }

        // Spawn background translation task
        let provider = Arc::clone(&self.provider);
        let jobs = Arc::clone(&self.jobs);
        let id = job_id.clone();

        tokio::spawn(async move {
            let result = execute_translation(&*provider, request).await;
            let status = match result {
                Ok(translations) => JobStatus::Complete { translations },
                Err(e) => JobStatus::Failed { error: e },
            };
            let mut jobs = jobs.write().expect("lock not poisoned");
            jobs.insert(id, status);
        });

        job_id
    }

    /// Get the status of a job.
    ///
    /// Returns `None` if the job ID doesn't exist (never submitted or already removed).
    pub fn get(&self, job_id: &str) -> Option<JobStatus> {
        let jobs = self.jobs.read().expect("lock not poisoned");
        jobs.get(job_id).cloned()
    }

    /// Remove a job from storage.
    ///
    /// Returns the final status if the job existed.
    /// Use this to clean up after processing results.
    pub fn remove(&self, job_id: &str) -> Option<JobStatus> {
        let mut jobs = self.jobs.write().expect("lock not poisoned");
        jobs.remove(job_id)
    }

    /// List all job IDs.
    ///
    /// Useful for debugging and admin interfaces.
    pub fn list_jobs(&self) -> Vec<JobId> {
        let jobs = self.jobs.read().expect("lock not poisoned");
        jobs.keys().cloned().collect()
    }
}

/// Execute translation for a job request.
async fn execute_translation<P: TranslationProvider>(
    provider: &P,
    request: TranslationRequest,
) -> Result<TranslationResult, String> {
    let mut results = HashMap::new();

    for (col_name, tgt_lang) in request.targets {
        let translated = provider
            .translate_batch(&request.src_lang, &tgt_lang, &request.texts)
            .await
            .map_err(|e| e.to_string())?;

        results.insert(col_name, translated);
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::mock::MockProvider;
    use std::time::Duration;

    // Helper to wait for job completion
    async fn wait_for_completion(manager: &JobManager<MockProvider>, job_id: &str) -> JobStatus {
        for _ in 0..100 {
            if let Some(status) = manager.get(job_id) {
                if !matches!(status, JobStatus::Pending) {
                    return status;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("Job did not complete within timeout");
    }

    #[tokio::test]
    async fn submit_returns_job_id_immediately() {
        let manager = JobManager::new(MockProvider);

        let job_id = manager.submit(TranslationRequest {
            texts: vec!["Hello".into()],
            src_lang: "eng_Latn".into(),
            targets: vec![("fr".into(), "fra_Latn".into())],
        });

        assert!(!job_id.is_empty());
        assert!(manager.get(&job_id).is_some());
    }

    #[tokio::test]
    async fn job_completes_with_translations_for_each_target() {
        let manager = JobManager::new(MockProvider);

        let job_id = manager.submit(TranslationRequest {
            texts: vec!["Hello".into(), "World".into()],
            src_lang: "eng_Latn".into(),
            targets: vec![
                ("fr".into(), "fra_Latn".into()),
                ("es".into(), "spa_Latn".into()),
            ],
        });

        let status = wait_for_completion(&manager, &job_id).await;

        match status {
            JobStatus::Complete { translations } => {
                assert_eq!(translations.len(), 2, "Should have 2 target languages");
                assert_eq!(translations["fr"].len(), 2, "French should have 2 texts");
                assert_eq!(translations["es"].len(), 2, "Spanish should have 2 texts");
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn remove_deletes_job_and_returns_status() {
        let manager = JobManager::new(MockProvider);

        let job_id = manager.submit(TranslationRequest {
            texts: vec!["Test".into()],
            src_lang: "eng_Latn".into(),
            targets: vec![("fr".into(), "fra_Latn".into())],
        });

        wait_for_completion(&manager, &job_id).await;

        let removed = manager.remove(&job_id);
        assert!(removed.is_some(), "Should return removed status");
        assert!(manager.get(&job_id).is_none(), "Job should be gone");
    }

    #[tokio::test]
    async fn nonexistent_job_returns_none() {
        let manager = JobManager::new(MockProvider);
        assert!(manager.get("nonexistent-job-id").is_none());
    }

    #[tokio::test]
    async fn empty_input_produces_empty_output() {
        let manager = JobManager::new(MockProvider);

        let job_id = manager.submit(TranslationRequest {
            texts: vec![],
            src_lang: "eng_Latn".into(),
            targets: vec![("fr".into(), "fra_Latn".into())],
        });

        let status = wait_for_completion(&manager, &job_id).await;

        match status {
            JobStatus::Complete { translations } => {
                assert!(translations["fr"].is_empty());
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn concurrent_jobs_complete_independently() {
        let manager = JobManager::new(MockProvider);

        let job_ids: Vec<JobId> = (0..5)
            .map(|i| {
                manager.submit(TranslationRequest {
                    texts: vec![format!("Text {i}")],
                    src_lang: "eng_Latn".into(),
                    targets: vec![("fr".into(), "fra_Latn".into())],
                })
            })
            .collect();

        // All IDs should be unique
        let unique_count = job_ids.iter().collect::<std::collections::HashSet<_>>().len();
        assert_eq!(unique_count, 5);

        // All should complete
        for job_id in &job_ids {
            let status = wait_for_completion(&manager, job_id).await;
            assert!(matches!(status, JobStatus::Complete { .. }));
        }
    }

    #[tokio::test]
    async fn list_jobs_returns_all_job_ids() {
        let manager = JobManager::new(MockProvider);

        let job1 = manager.submit(TranslationRequest {
            texts: vec!["A".into()],
            src_lang: "eng_Latn".into(),
            targets: vec![("fr".into(), "fra_Latn".into())],
        });

        let job2 = manager.submit(TranslationRequest {
            texts: vec!["B".into()],
            src_lang: "eng_Latn".into(),
            targets: vec![("fr".into(), "fra_Latn".into())],
        });

        let list = manager.list_jobs();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&job1));
        assert!(list.contains(&job2));
    }

    #[tokio::test]
    async fn multiple_target_languages_are_all_translated() {
        let manager = JobManager::new(MockProvider);

        let targets = vec![
            ("french".into(), "fra_Latn".into()),
            ("spanish".into(), "spa_Latn".into()),
            ("german".into(), "deu_Latn".into()),
            ("italian".into(), "ita_Latn".into()),
            ("portuguese".into(), "por_Latn".into()),
        ];

        let job_id = manager.submit(TranslationRequest {
            texts: vec!["Hello".into()],
            src_lang: "eng_Latn".into(),
            targets: targets.clone(),
        });

        let status = wait_for_completion(&manager, &job_id).await;

        match status {
            JobStatus::Complete { translations } => {
                assert_eq!(translations.len(), 5);
                for (col_name, _) in &targets {
                    assert!(
                        translations.contains_key(col_name),
                        "Missing column: {col_name}"
                    );
                }
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }
}
