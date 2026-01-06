# Chapter 1: Understanding the Existing Crate

**Duration**: 2 hours
**Sections**: 4 (30 minutes each)

## Overview

Before adding new features, you need to understand what already exists. This chapter walks through the `translator` library crate, examining each module and how the pieces connect.

By the end, you will:

- Understand the `TranslationProvider` trait contract
- Know how `JobManager` handles async translation jobs
- Trace the CSV pipeline flow
- Recognize the error handling patterns

---

## Section 1.1: The Provider Trait (30 min)

### Learning Objective

Understand the `TranslationProvider` trait that makes the library extensible.

### Available Providers

The library includes three translation providers:

| Provider | Purpose | Requirements |
|----------|---------|--------------|
| `MockProvider` | Testing and development | None |
| `NllbRestProvider` | Production with 200+ languages | NLLB server |
| `MarianProvider` | Offline, pure Rust | libtorch + `marian` feature |

### The Trait Contract

Open `src/provider/mod.rs`. The `TranslationProvider` trait defines what all translation backends must implement:

```rust
// File: src/provider/mod.rs

pub trait TranslationProvider: Send + Sync + 'static {
    fn translate_batch(
        &self,
        src_lang: &str,
        tgt_lang: &str,
        texts: &[String],
    ) -> impl Future<Output = Result<Vec<String>, TranslateError>> + Send;

    fn name(&self) -> &'static str;
}
```

**Why these design choices?**

| Element | Reason |
|---------|--------|
| `Send + Sync + 'static` | Providers must be thread-safe for concurrent async tasks |
| `&[String]` input | Batch processing for efficient API calls |
| `&str` language codes | Provider-agnostic (different providers support different languages) |
| `impl Future` | Uses Rust 1.75 async fn in traits for zero-cost abstraction |

### Provider Contract Rules

Implementations MUST:

1. **Preserve order**: Translation at index `i` corresponds to input at index `i`
2. **Match length**: Return exactly `texts.len()` translations
3. **Handle empty input**: Return empty `Vec` without network calls

Violating these rules causes `TranslateError::TranslationCountMismatch`.

### MockProvider Implementation

Open `src/provider/mock.rs`:

```rust
// File: src/provider/mock.rs

#[derive(Clone, Copy, Debug, Default)]
pub struct MockProvider;

impl TranslationProvider for MockProvider {
    async fn translate_batch(
        &self,
        _src_lang: &str,
        tgt_lang: &str,
        texts: &[String],
    ) -> Result<Vec<String>, TranslateError> {
        let translated = texts
            .iter()
            .map(|text| format!("[mock::{tgt_lang}] {text}"))
            .collect();

        Ok(translated)
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}
```

Key observations:

- **Zero-sized struct** (`Clone, Copy`): No allocation to use
- **Ignores `src_lang`**: Real providers would validate it
- **Wraps with marker**: Useful for verifying the pipeline works

### Verification: Section 1.1

```bash
cargo test -p translator mock
```

**Expected**: All mock provider tests pass (3-4 tests)

**Check your understanding**:
- Q: What happens if a provider returns fewer translations than inputs?
- A: `TranslateError::TranslationCountMismatch` is returned

---

## Section 1.2: The NllbRestProvider (30 min)

### Learning Objective

Understand how a production provider implements retry logic and error handling.

### Builder Pattern Configuration

Open `src/provider/nllb_rest.rs`. The provider uses the builder pattern:

```rust
// File: src/provider/nllb_rest.rs

pub struct NllbRestProvider {
    client: Client,
    base_url: String,
    timeout: Duration,
    max_retries: u32,
    backoff_base: Duration,
    backoff_cap: Duration,
}

impl NllbRestProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            timeout: defaults::TIMEOUT,       // 30s
            max_retries: defaults::MAX_RETRIES, // 3
            backoff_base: defaults::BACKOFF_BASE, // 250ms
            backoff_cap: defaults::BACKOFF_CAP,   // 4s
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    // ... more builder methods
}
```

Usage examples:

```rust
// Minimal configuration
let provider = NllbRestProvider::new("http://localhost:8080");

// Full configuration
let provider = NllbRestProvider::new("http://localhost:8080")
    .with_timeout(Duration::from_secs(60))
    .with_max_retries(5);
```

### Retry Logic

The provider implements exponential backoff with jitter:

| Attempt | Delay Range |
|---------|-------------|
| 0 | 250-350ms |
| 1 | 500-600ms |
| 2 | 1000-1100ms |
| 3 | 2000-2100ms |

Retryable conditions:

- **Status codes**: 408, 429, 500, 502, 503, 504
- **Network errors**: Timeout, connection refused

The jitter (random 0-100ms) prevents thundering herd when multiple clients retry simultaneously.

### Backoff Calculation

```rust
fn calculate_backoff(attempt: u32, base: Duration, cap: Duration) -> Duration {
    let multiplier = 1u64 << attempt.min(10); // 2^attempt, max 2^10
    let base_ms = base.as_millis() as u64;
    let delay_ms = base_ms.saturating_mul(multiplier);
    let capped_ms = delay_ms.min(cap.as_millis() as u64);
    let jitter_ms = fastrand::u64(0..=100);
    Duration::from_millis(capped_ms + jitter_ms)
}
```

### Verification: Section 1.2

```bash
cargo test -p translator nllb_rest
```

**Expected**: All backoff and status tests pass

**Check your understanding**:
- Q: What happens if NLLB returns HTTP 400?
- A: Returns immediately with Provider error (400 is not retryable)
- Q: What is the max delay between retries?
- A: 4 seconds + 100ms jitter = 4.1 seconds max

---

## Section 1.3: The Job Manager (30 min)

### Learning Objective

Understand how `JobManager` provides async job handling for web service integration.

### Architecture

Open `src/jobs.rs`. The `JobManager` wraps a provider and manages job state:

```rust
// File: src/jobs.rs

#[derive(Clone)]
pub struct JobManager<P> {
    provider: Arc<P>,
    jobs: Arc<RwLock<HashMap<JobId, JobStatus>>>,
}
```

| Element | Purpose |
|---------|---------|
| `JobManager<P>` | Generic over any `TranslationProvider` |
| `Arc<P>` | Cheap cloning for sharing across handlers |
| `Arc<RwLock<...>>` | Thread-safe job storage |

### Job Lifecycle

```
submit() -> Pending -> [background task] -> Complete/Failed
                              |
                              v
                        get() -> returns current status
                              |
                              v
                        remove() -> cleans up
```

### The Submit Method

```rust
// File: src/jobs.rs

pub fn submit(&self, request: TranslationRequest) -> JobId {
    let job_id = Uuid::new_v4().to_string();

    // Mark as pending BEFORE spawning (avoid race condition)
    {
        let mut jobs = self.jobs.write().expect("lock not poisoned");
        jobs.insert(job_id.clone(), JobStatus::Pending);
    }

    // Spawn background task
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
```

Critical patterns:

| Pattern | Reason |
|---------|--------|
| Insert Pending before spawn | Prevents race where `get()` returns None immediately after `submit()` |
| Arc cloning | Cheap reference counting, not deep copy |
| Move semantics | Spawned task owns what it needs |
| Fire and forget | `tokio::spawn` returns immediately |

### Job Status Enum

```rust
pub enum JobStatus {
    Pending,
    Complete { translations: TranslationResult },
    Failed { error: String },
}
```

`TranslationResult` is `HashMap<String, Vec<String>>` mapping column names to translations.

### Memory Management

From the module documentation:

> Completed jobs remain in memory until explicitly removed with `remove()`.
> For long-running services, implement periodic cleanup or TTL-based expiration.

This is why Chapter 3 adds Redis job storage. The current in-memory storage does not scale.

### Verification: Section 1.3

```bash
cargo test -p translator jobs
```

**Expected**: All job manager tests pass (8+ tests)

**Check your understanding**:
- Q: Why insert Pending before spawning the task?
- A: Prevents `get()` from returning None if called immediately after `submit()`
- Q: What happens to completed jobs if `remove()` is never called?
- A: They stay in memory forever (memory leak in long-running services)

---

## Section 1.4: Error Handling Patterns (30 min)

### Learning Objective

Understand the `TranslateError` enum and how errors flow through the system.

### The Error Enum

Open `src/error.rs`:

```rust
// File: src/error.rs

#[derive(Debug, Error)]
pub enum TranslateError {
    #[error("File operation failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("CSV processing failed: {0}")]
    Csv(#[from] csv_async::Error),

    #[error("Column '{column}' not found in CSV. Available columns: {available:?}. \
             Use --source-col to specify the correct column name.")]
    ColumnNotFound { column: String, available: Vec<String> },

    #[error("Translation count mismatch: expected {expected}, got {actual}. \
             This is a provider bug - please report it.")]
    TranslationCountMismatch { expected: usize, actual: usize },

    #[error("Unsupported language code: '{code}'. Supported codes: {supported:?}")]
    UnsupportedLanguage { code: String, supported: Vec<String> },

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("All {attempts} retry attempts exhausted. Last error: {last_error}. \
             Consider increasing --timeout-ms or checking server health.")]
    RetriesExhausted { attempts: u32, last_error: String },
}
```

### Design Principles

| Principle | Example |
|-----------|---------|
| `#[from]` for auto-conversion | `Io(#[from] std::io::Error)` enables `?` operator |
| Actionable messages | `"Use --source-col to specify..."` |
| Structured variants | `ColumnNotFound` includes available columns |
| No heap allocation when possible | Simple variants avoid `String` |

### Constructor Helpers

```rust
impl TranslateError {
    pub fn column_not_found(column: impl Into<String>, available: Vec<String>) -> Self {
        Self::ColumnNotFound { column: column.into(), available }
    }

    pub fn count_mismatch(expected: usize, actual: usize) -> Self {
        Self::TranslationCountMismatch { expected, actual }
    }
}
```

More ergonomic:

```rust
// Instead of:
Err(TranslateError::ColumnNotFound {
    column: "foo".to_string(),
    available: vec![]
})

// Write:
Err(TranslateError::column_not_found("foo", vec![]))
```

### The is_retryable Method

```rust
impl TranslateError {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http(e) => e.is_timeout() || e.is_connect() || e.is_request(),
            Self::Provider(_) => true,  // Might be rate limiting
            Self::RetriesExhausted { .. } => false,
            Self::ColumnNotFound { .. } => false,
            Self::UnsupportedLanguage { .. } => false,
            Self::TranslationCountMismatch { .. } => false,
            Self::Io(_) => false,
            Self::Csv(_) => false,
        }
    }
}
```

This enables higher-level retry logic. HTTP handlers can tell clients whether to retry.

### Error to HTTP Status Mapping

| Error Variant | HTTP Status | Reason |
|---------------|-------------|--------|
| `Io` | 500 | Server problem |
| `Http` | 502 | Upstream provider problem |
| `Csv` | 400 | Client sent bad data |
| `ColumnNotFound` | 400 | Client configuration error |
| `TranslationCountMismatch` | 500 | Provider bug |
| `UnsupportedLanguage` | 400 | Client used invalid language code |
| `Provider` | 502 | Upstream provider problem |
| `RetriesExhausted` | 503 | Temporary unavailability |

You will implement this mapping in Chapter 2.

### Verification: Section 1.4

```bash
cargo test -p translator error
```

**Expected**: All error tests pass (4 tests)

**Check your understanding**:
- Q: Why is `Provider(_)` considered retryable?
- A: Provider errors might be rate limiting or temporary issues
- Q: Why include `"--source-col"` in the ColumnNotFound message?
- A: Users can immediately see how to fix the problem

---

## Chapter Summary

You now understand:

| Component | Purpose |
|-----------|---------|
| `TranslationProvider` trait | Extensible abstraction for translation backends |
| `MockProvider` | Zero-cost testing provider |
| `NllbRestProvider` | Production provider with retry logic |
| `JobManager` | Async job handling for web services |
| `TranslateError` | Actionable errors with `thiserror` |

### Key Insights for Next Chapters

- `JobManager` stores jobs in memory. You need Redis for persistence.
- The provider trait is generic enough for HTTP handlers.
- Error variants map cleanly to HTTP status codes.
- The library is designed for batch processing, not streaming.

---

## Next Chapter

Continue to [Chapter 2: Creating the Axum Workspace](./02-axum-workspace.md) to:

- Convert to a Cargo workspace
- Create the `translator-server` crate
- Build HTTP endpoints that use the library
