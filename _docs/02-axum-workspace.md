# Chapter 2: Creating the Axum Workspace

**Duration**: 2 hours
**Sections**: 4 (30 minutes each)

## Overview

In this chapter, you transform the single-crate project into a Cargo workspace and create a new `translator-server` binary that exposes the translation library via HTTP.

By the end, you will have:

- A workspace with `translator` (library) and `translator-server` (binary)
- Health check endpoint
- Translation submission endpoint
- Job status polling endpoint
- Graceful shutdown and request logging

---

## Section 2.1: Converting to a Workspace (30 min)

### Learning Objective

Restructure the project as a Cargo workspace with two members.

### Create the Workspace Structure

```bash
cd /path/to/csv-translation-pipeline

# Create the translator-server crate directory
mkdir -p translator-server/src

# Create translator subdirectory
mkdir translator

# Move source files
mv src translator/
mv Cargo.toml translator/

# Move other library files if they exist
mv tests translator/ 2>/dev/null || true
mv examples translator/ 2>/dev/null || true
```

### Create Workspace Root Cargo.toml

```toml
# File: Cargo.toml (workspace root)

[workspace]
resolver = "2"
members = [
    "translator",
    "translator-server",
]

[workspace.package]
edition = "2021"
rust-version = "1.75"
license = "MIT"
repository = "https://github.com/yourorg/csv-translation-pipeline"

[workspace.dependencies]
# Shared dependencies - version managed here
tokio = { version = "1", features = ["rt-multi-thread", "macros", "fs", "time", "signal"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
uuid = { version = "1", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Axum and tower ecosystem
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["trace", "cors"] }

# HTTP client
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

# The translator library itself
translator = { path = "translator" }
```

### Update translator/Cargo.toml

```toml
# File: translator/Cargo.toml

[package]
name = "translator"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "Async CSV translation pipeline with pluggable providers"

[[bin]]
name = "translate_csv"
path = "src/bin/translate_csv.rs"

[dependencies]
tokio.workspace = true
serde.workspace = true
reqwest.workspace = true
thiserror.workspace = true
uuid.workspace = true

csv-async = { version = "1", features = ["tokio"] }
clap = { version = "4", features = ["derive"] }
fastrand = "2"

# Optional: MarianMT provider
rust-bert = { version = "0.23", optional = true }
tch = { version = "0.17", optional = true }

[dev-dependencies]
tokio-test = "0.4"
csv = "1"

[features]
default = []
marian = ["rust-bert", "tch"]
```

### Create translator-server/Cargo.toml

```toml
# File: translator-server/Cargo.toml

[package]
name = "translator-server"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "HTTP server for the CSV translation pipeline"

[[bin]]
name = "translator-server"
path = "src/main.rs"

[dependencies]
translator.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
axum.workspace = true
tower.workspace = true
tower-http.workspace = true

[dev-dependencies]
reqwest = { workspace = true, features = ["json"] }
```

### Create Initial main.rs

```rust
// File: translator-server/src/main.rs

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let app = Router::new().route("/", get(root));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "Translation Server"
}
```

### Verification: Section 2.1

```bash
cargo build
```

**Expected**: Both workspace members compile successfully

```bash
cargo test -p translator
```

**Expected**: All library tests pass

```bash
cargo run -p translator-server &
curl http://localhost:3000/
kill %1
```

**Expected**: Returns "Translation Server"

---

## Section 2.2: Application State and Health Check (30 min)

### Learning Objective

Create shared application state and a health check endpoint.

### Create the State Module

```rust
// File: translator-server/src/state.rs

use std::sync::Arc;
use translator::{provider::mock::MockProvider, JobManager};

/// Shared application state.
/// Cloned (cheaply via Arc) into each request handler.
#[derive(Clone)]
pub struct AppState {
    pub jobs: JobManager<MockProvider>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            jobs: JobManager::new(MockProvider),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedState = Arc<AppState>;
```

### Create the Routes Module Structure

```bash
mkdir -p translator-server/src/routes
```

```rust
// File: translator-server/src/routes/mod.rs

pub mod health;

use axum::Router;
use crate::state::SharedState;

pub fn create_router(state: SharedState) -> Router {
    Router::new()
        .merge(health::router())
        .with_state(state)
}
```

### Create the Health Check Route

```rust
// File: translator-server/src/routes/health.rs

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;
use crate::state::SharedState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

pub async fn health_check(State(_state): State<SharedState>) -> impl IntoResponse {
    let response = HealthResponse {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
    };
    (StatusCode::OK, Json(response))
}

pub fn router() -> Router<SharedState> {
    Router::new().route("/health", get(health_check))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use std::sync::Arc;
    use tower::ServiceExt;
    use crate::state::AppState;

    #[tokio::test]
    async fn health_check_returns_healthy() {
        let state = Arc::new(AppState::new());
        let app = router().with_state(state);

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["status"], "healthy");
    }
}
```

### Update main.rs

```rust
// File: translator-server/src/main.rs

mod routes;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use crate::state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "translator_server=debug,tower_http=debug".into()),
        )
        .init();

    let state = Arc::new(AppState::new());
    let app = routes::create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

### Verification: Section 2.2

```bash
cargo test -p translator-server
```

**Expected**: `health_check_returns_healthy` passes

```bash
cargo run -p translator-server &
curl -s http://localhost:3000/health | jq
kill %1
```

**Expected output**:

```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

---

## Section 2.3: Translation Submission Endpoint (30 min)

### Learning Objective

Create a POST endpoint to submit translation jobs.

### Create the Translate Routes Module

```rust
// File: translator-server/src/routes/translate.rs

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use translator::TranslationRequest;
use crate::state::SharedState;

#[derive(Debug, Deserialize)]
pub struct SubmitTranslationRequest {
    pub texts: Vec<String>,
    pub src_lang: String,
    pub targets: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct SubmitTranslationResponse {
    pub job_id: String,
    pub status_url: String,
}

pub async fn submit_translation(
    State(state): State<SharedState>,
    Json(payload): Json<SubmitTranslationRequest>,
) -> Result<impl IntoResponse, TranslateApiError> {
    if payload.texts.is_empty() {
        return Err(TranslateApiError::BadRequest(
            "texts array cannot be empty".into(),
        ));
    }

    if payload.targets.is_empty() {
        return Err(TranslateApiError::BadRequest(
            "targets cannot be empty".into(),
        ));
    }

    let targets: Vec<(String, String)> = payload
        .targets
        .into_iter()
        .map(|(col, lang)| (col, lang))
        .collect();

    let request = TranslationRequest {
        texts: payload.texts,
        src_lang: payload.src_lang,
        targets,
    };

    let job_id = state.jobs.submit(request);

    let response = SubmitTranslationResponse {
        status_url: format!("/jobs/{}", job_id),
        job_id,
    };

    Ok((StatusCode::ACCEPTED, Json(response)))
}

#[derive(Debug)]
pub enum TranslateApiError {
    BadRequest(String),
    NotFound(String),
    InternalError(String),
}

impl IntoResponse for TranslateApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            Self::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({ "error": message }));
        (status, body).into_response()
    }
}

pub fn router() -> Router<SharedState> {
    Router::new().route("/translate", post(submit_translation))
}
```

### Update the Routes Module

```rust
// File: translator-server/src/routes/mod.rs

pub mod health;
pub mod translate;

use axum::Router;
use crate::state::SharedState;

pub fn create_router(state: SharedState) -> Router {
    Router::new()
        .merge(health::router())
        .merge(translate::router())
        .with_state(state)
}
```

### Verification: Section 2.3

```bash
cargo test -p translator-server translate
```

**Expected**: Both translation tests pass

```bash
cargo run -p translator-server &
curl -X POST http://localhost:3000/translate \
  -H "Content-Type: application/json" \
  -d '{"texts":["Hello"],"src_lang":"eng_Latn","targets":{"fr":"fra_Latn"}}' | jq
kill %1
```

**Expected output** (job_id will vary):

```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "status_url": "/jobs/550e8400-e29b-41d4-a716-446655440000"
}
```

---

## Section 2.4: Job Status and Production Features (30 min)

### Learning Objective

Create a GET endpoint for job status and add production-ready features.

### Create the Jobs Routes Module

```rust
// File: translator-server/src/routes/jobs.rs

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get},
    Json, Router,
};
use serde::Serialize;
use translator::JobStatus;
use crate::state::SharedState;
use super::translate::TranslateApiError;

#[derive(Debug, Serialize)]
#[serde(tag = "status")]
pub enum JobStatusResponse {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "complete")]
    Complete {
        translations: std::collections::HashMap<String, Vec<String>>,
    },
    #[serde(rename = "failed")]
    Failed { error: String },
}

impl From<JobStatus> for JobStatusResponse {
    fn from(status: JobStatus) -> Self {
        match status {
            JobStatus::Pending => Self::Pending,
            JobStatus::Complete { translations } => Self::Complete { translations },
            JobStatus::Failed { error } => Self::Failed { error },
        }
    }
}

pub async fn get_job_status(
    State(state): State<SharedState>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, TranslateApiError> {
    let status = state
        .jobs
        .get(&job_id)
        .ok_or_else(|| TranslateApiError::NotFound(format!("Job {} not found", job_id)))?;

    let response: JobStatusResponse = status.into();
    Ok(Json(response))
}

pub async fn delete_job(
    State(state): State<SharedState>,
    Path(job_id): Path<String>,
) -> Result<impl IntoResponse, TranslateApiError> {
    state
        .jobs
        .remove(&job_id)
        .ok_or_else(|| TranslateApiError::NotFound(format!("Job {} not found", job_id)))?;

    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/jobs/{id}", get(get_job_status))
        .route("/jobs/{id}", delete(delete_job))
}
```

### Update Routes with CORS

```rust
// File: translator-server/src/routes/mod.rs

pub mod health;
pub mod jobs;
pub mod translate;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use crate::state::SharedState;

pub fn create_router(state: SharedState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .merge(health::router())
        .merge(translate::router())
        .merge(jobs::router())
        .layer(cors)
        .with_state(state)
}
```

### Add Graceful Shutdown

```rust
// File: translator-server/src/main.rs

mod routes;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use crate::state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "translator_server=debug,tower_http=debug".into()),
        )
        .init();

    let state = Arc::new(AppState::new());

    let app = routes::create_router(state)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    tracing::info!("Server shutdown complete");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received Ctrl+C, starting graceful shutdown"),
        _ = terminate => tracing::info!("Received SIGTERM, starting graceful shutdown"),
    }
}
```

### Verification: Section 2.4

```bash
cargo test -p translator-server jobs
```

**Expected**: Both job tests pass

**Manual test flow**:

```bash
# Terminal 1: Start server
cargo run -p translator-server

# Terminal 2: Submit and poll
JOB_ID=$(curl -s -X POST http://localhost:3000/translate \
  -H "Content-Type: application/json" \
  -d '{"texts":["Hello","World"],"src_lang":"eng_Latn","targets":{"fr":"fra_Latn"}}' \
  | jq -r '.job_id')

echo "Job ID: $JOB_ID"

curl -s "http://localhost:3000/jobs/$JOB_ID" | jq
```

**Expected output**:

```json
{
  "status": "complete",
  "translations": {
    "fr": ["[mock::fra_Latn] Hello", "[mock::fra_Latn] World"]
  }
}
```

**Test graceful shutdown**: Press Ctrl+C in Terminal 1. You should see:
- "Received Ctrl+C, starting graceful shutdown"
- "Server shutdown complete"

---

## Chapter Summary

You now have a working Axum web server:

| Feature | Implementation |
|---------|----------------|
| Workspace structure | `translator` library + `translator-server` binary |
| Health check | `GET /health` for load balancer probes |
| Translation submission | `POST /translate` returns job ID |
| Job polling | `GET /jobs/{id}` returns status and results |
| Job cleanup | `DELETE /jobs/{id}` removes completed jobs |
| Request logging | Tower HTTP tracing layer |
| Graceful shutdown | Handles Ctrl+C and SIGTERM |
| CORS | Allows cross-origin requests for web UI |

### API Summary

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| POST | `/translate` | Submit translation job |
| GET | `/jobs/{id}` | Get job status |
| DELETE | `/jobs/{id}` | Remove job |

### Current Limitations

- Jobs stored in memory (lost on restart)
- No translation caching (every request hits the provider)
- Mock provider only (no real translations)

---

## Next Chapter

Continue to [Chapter 3: Adding Redis for Caching and Jobs](./03-redis-integration.md) to:

- Add Redis for job persistence
- Implement translation caching
- Add cache hit/miss metrics
