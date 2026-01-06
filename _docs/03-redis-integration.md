# Chapter 3: Adding Redis for Caching and Jobs

**Duration**: 2 hours
**Sections**: 4 (30 minutes each)

## Overview

In this chapter, you integrate Redis for two critical production features:

1. **Translation caching**: Avoid redundant ML model calls
2. **Job persistence**: Store jobs in Redis instead of in-memory HashMap

By the end, you will have:

- Redis connection pool in application state
- Translation cache with configurable TTL
- Redis-backed job storage
- Cache statistics endpoint

---

## Prerequisites

- Completed Chapter 2
- Redis server running locally

### Starting Redis

```bash
# macOS with Homebrew
brew install redis
brew services start redis

# Linux
sudo apt install redis-server
sudo systemctl start redis

# Docker (any platform)
docker run -d --name redis -p 6379:6379 redis:7
```

### Verification: Redis Running

```bash
redis-cli ping
```

**Expected**: `PONG`

---

## Section 3.1: Redis Connection Pool (30 min)

### Learning Objective

Add Redis connection pooling to the application state.

### Add Redis Dependencies

Update workspace root `Cargo.toml`:

```toml
# File: Cargo.toml (add to [workspace.dependencies])

redis = { version = "0.27", features = ["tokio-comp", "connection-manager"] }
sha2 = "0.10"
```

Update `translator-server/Cargo.toml`:

```toml
# File: translator-server/Cargo.toml (add to [dependencies])

redis.workspace = true
sha2 = "0.10"
```

### Create Config Module

```rust
// File: translator-server/src/config.rs

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub redis_url: String,
    pub port: u16,
    pub cache_ttl_seconds: u64,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            cache_ttl_seconds: env::var("CACHE_TTL_SECONDS")
                .ok()
                .and_then(|t| t.parse().ok())
                .unwrap_or(86400), // 24 hours
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}
```

### Update Application State

```rust
// File: translator-server/src/state.rs

use std::sync::Arc;
use redis::aio::MultiplexedConnection;
use translator::{provider::mock::MockProvider, JobManager};
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub jobs: JobManager<MockProvider>,
    pub redis: MultiplexedConnection,
    pub config: Config,
}

impl AppState {
    pub async fn new(config: Config) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(config.redis_url.as_str())?;
        let redis = client.get_multiplexed_async_connection().await?;

        Ok(Self {
            jobs: JobManager::new(MockProvider),
            redis,
            config,
        })
    }
}

pub type SharedState = Arc<AppState>;
```

### Update main.rs

```rust
// File: translator-server/src/main.rs

mod config;
mod routes;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use crate::config::Config;
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

    let config = Config::from_env();
    tracing::info!("Redis URL: {}", config.redis_url);

    let state = Arc::new(
        AppState::new(config.clone())
            .await
            .expect("Failed to connect to Redis"),
    );
    tracing::info!("Connected to Redis");

    let app = routes::create_router(state)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
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

### Verification: Section 3.1

```bash
cargo build -p translator-server
```

**Expected**: Compiles successfully

With Redis running:

```bash
cargo run -p translator-server
```

**Expected**: Logs "Connected to Redis"

---

## Section 3.2: Translation Cache (30 min)

### Learning Objective

Create a Redis-backed translation cache to avoid duplicate ML calls.

### Create the Cache Module

```rust
// File: translator-server/src/cache.rs

use redis::AsyncCommands;
use sha2::{Digest, Sha256};
use crate::state::SharedState;

/// Generate a cache key for a translation.
/// Key format: `translation:{sha256(text)}:{src_lang}:{tgt_lang}`
fn cache_key(text: &str, src_lang: &str, tgt_lang: &str) -> String {
    let hash = Sha256::digest(text.as_bytes());
    format!("translation:{:x}:{}:{}", hash, src_lang, tgt_lang)
}

pub struct TranslationCache;

impl TranslationCache {
    /// Get a cached translation if it exists.
    pub async fn get(
        state: &SharedState,
        text: &str,
        src_lang: &str,
        tgt_lang: &str,
    ) -> Option<String> {
        let key = cache_key(text, src_lang, tgt_lang);
        let result: Result<Option<String>, _> = state.redis.clone().get(&key).await;

        match result {
            Ok(Some(cached)) => {
                tracing::debug!(key = %key, "Cache HIT");
                Some(cached)
            }
            Ok(None) => {
                tracing::debug!(key = %key, "Cache MISS");
                None
            }
            Err(e) => {
                tracing::warn!(error = %e, "Redis GET failed");
                None
            }
        }
    }

    /// Store a translation in the cache.
    pub async fn set(
        state: &SharedState,
        text: &str,
        src_lang: &str,
        tgt_lang: &str,
        translation: &str,
    ) {
        let key = cache_key(text, src_lang, tgt_lang);
        let ttl = state.config.cache_ttl_seconds;

        let result: Result<(), _> = state
            .redis
            .clone()
            .set_ex(&key, translation, ttl)
            .await;

        if let Err(e) = result {
            tracing::warn!(error = %e, "Redis SET failed");
        }
    }

    /// Get multiple cached translations at once using MGET.
    pub async fn get_batch(
        state: &SharedState,
        texts: &[String],
        src_lang: &str,
        tgt_lang: &str,
    ) -> Vec<Option<String>> {
        if texts.is_empty() {
            return Vec::new();
        }

        let keys: Vec<String> = texts
            .iter()
            .map(|t| cache_key(t, src_lang, tgt_lang))
            .collect();

        let result: Result<Vec<Option<String>>, _> = state.redis.clone().mget(&keys).await;

        match result {
            Ok(values) => {
                let hits = values.iter().filter(|v| v.is_some()).count();
                tracing::debug!(hits = hits, misses = values.len() - hits, "Batch cache lookup");
                values
            }
            Err(e) => {
                tracing::warn!(error = %e, "Redis MGET failed");
                vec![None; texts.len()]
            }
        }
    }

    /// Store multiple translations using Redis pipeline.
    pub async fn set_batch(
        state: &SharedState,
        texts: &[String],
        src_lang: &str,
        tgt_lang: &str,
        translations: &[String],
    ) {
        if texts.is_empty() {
            return;
        }

        let ttl = state.config.cache_ttl_seconds;
        let mut pipe = redis::pipe();

        for (text, translation) in texts.iter().zip(translations.iter()) {
            let key = cache_key(text, src_lang, tgt_lang);
            pipe.set_ex(&key, translation, ttl);
        }

        let result: Result<(), _> = pipe.query_async(&mut state.redis.clone()).await;

        if let Err(e) = result {
            tracing::warn!(error = %e, "Redis pipeline SET failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_deterministic() {
        let key1 = cache_key("Hello", "eng_Latn", "fra_Latn");
        let key2 = cache_key("Hello", "eng_Latn", "fra_Latn");
        assert_eq!(key1, key2);
    }

    #[test]
    fn cache_key_differs_for_different_texts() {
        let key1 = cache_key("Hello", "eng_Latn", "fra_Latn");
        let key2 = cache_key("World", "eng_Latn", "fra_Latn");
        assert_ne!(key1, key2);
    }

    #[test]
    fn cache_key_differs_for_different_languages() {
        let key1 = cache_key("Hello", "eng_Latn", "fra_Latn");
        let key2 = cache_key("Hello", "eng_Latn", "spa_Latn");
        assert_ne!(key1, key2);
    }
}
```

### Update main.rs

```rust
// File: translator-server/src/main.rs (add module declaration)

mod cache;
mod config;
mod routes;
mod state;
```

### Verification: Section 3.2

```bash
cargo test -p translator-server cache
```

**Expected**: All cache key tests pass (3 tests)

---

## Section 3.3: Redis Job Storage (30 min)

### Learning Objective

Store job metadata in Redis for persistence across restarts.

### Create Job Storage Module

```rust
// File: translator-server/src/job_storage.rs

use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::state::SharedState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredJob {
    pub texts: Vec<String>,
    pub src_lang: String,
    pub targets: HashMap<String, String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translations: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn job_key(job_id: &str) -> String {
    format!("job:{}", job_id)
}

pub struct JobStorage;

impl JobStorage {
    pub async fn store_pending(
        state: &SharedState,
        job_id: &str,
        texts: Vec<String>,
        src_lang: String,
        targets: HashMap<String, String>,
    ) {
        let job = StoredJob {
            texts,
            src_lang,
            targets,
            status: "pending".into(),
            translations: None,
            error: None,
        };

        let key = job_key(job_id);
        let value = serde_json::to_string(&job).unwrap_or_default();
        let ttl = 86400u64; // 24 hours

        let result: Result<(), _> = state.redis.clone().set_ex(&key, &value, ttl).await;

        if let Err(e) = result {
            tracing::warn!(error = %e, job_id = %job_id, "Failed to store job");
        }
    }

    pub async fn mark_complete(
        state: &SharedState,
        job_id: &str,
        translations: HashMap<String, Vec<String>>,
    ) {
        let key = job_key(job_id);
        let existing: Result<Option<String>, _> = state.redis.clone().get(&key).await;

        if let Ok(Some(value)) = existing {
            if let Ok(mut job) = serde_json::from_str::<StoredJob>(&value) {
                job.status = "complete".into();
                job.translations = Some(translations);
                let new_value = serde_json::to_string(&job).unwrap_or_default();
                let _: Result<(), _> = state.redis.clone().set(&key, &new_value).await;
            }
        }
    }

    pub async fn mark_failed(state: &SharedState, job_id: &str, error: String) {
        let key = job_key(job_id);
        let existing: Result<Option<String>, _> = state.redis.clone().get(&key).await;

        if let Ok(Some(value)) = existing {
            if let Ok(mut job) = serde_json::from_str::<StoredJob>(&value) {
                job.status = "failed".into();
                job.error = Some(error);
                let new_value = serde_json::to_string(&job).unwrap_or_default();
                let _: Result<(), _> = state.redis.clone().set(&key, &new_value).await;
            }
        }
    }

    pub async fn get(state: &SharedState, job_id: &str) -> Option<StoredJob> {
        let key = job_key(job_id);
        let result: Result<Option<String>, _> = state.redis.clone().get(&key).await;

        match result {
            Ok(Some(value)) => serde_json::from_str(&value).ok(),
            _ => None,
        }
    }

    pub async fn delete(state: &SharedState, job_id: &str) {
        let key = job_key(job_id);
        let _: Result<(), _> = state.redis.clone().del(&key).await;
    }
}
```

### Update Translation Handler

```rust
// File: translator-server/src/routes/translate.rs (update submit_translation)

use crate::job_storage::JobStorage;

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
        .iter()
        .map(|(col, lang)| (col.clone(), lang.clone()))
        .collect();

    let request = translator::TranslationRequest {
        texts: payload.texts.clone(),
        src_lang: payload.src_lang.clone(),
        targets,
    };

    let job_id = state.jobs.submit(request);

    // Store job metadata in Redis
    JobStorage::store_pending(
        &state,
        &job_id,
        payload.texts,
        payload.src_lang,
        payload.targets,
    )
    .await;

    let response = SubmitTranslationResponse {
        status_url: format!("/jobs/{}", job_id),
        job_id,
    };

    Ok((StatusCode::ACCEPTED, Json(response)))
}
```

### Update main.rs

```rust
// File: translator-server/src/main.rs

mod cache;
mod config;
mod job_storage;
mod routes;
mod state;
```

### Verification: Section 3.3

```bash
cargo run -p translator-server &

JOB_ID=$(curl -s -X POST http://localhost:3000/translate \
  -H "Content-Type: application/json" \
  -d '{"texts":["Hello"],"src_lang":"eng_Latn","targets":{"fr":"fra_Latn"}}' \
  | jq -r '.job_id')

redis-cli GET "job:$JOB_ID"
kill %1
```

**Expected**: JSON with job metadata including status field

---

## Section 3.4: Statistics Endpoint (30 min)

### Learning Objective

Add an endpoint to monitor cache and job statistics.

### Create Stats Module

```rust
// File: translator-server/src/routes/stats.rs

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use redis::AsyncCommands;
use serde::Serialize;
use crate::state::SharedState;

#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub translation_count: u64,
    pub job_count: u64,
    pub redis_connected: bool,
}

pub async fn get_stats(State(state): State<SharedState>) -> impl IntoResponse {
    let translation_keys: Result<Vec<String>, _> = state
        .redis
        .clone()
        .keys::<_, Vec<String>>("translation:*")
        .await;

    let translation_count = translation_keys.map(|k| k.len() as u64).unwrap_or(0);

    let job_keys: Result<Vec<String>, _> = state
        .redis
        .clone()
        .keys::<_, Vec<String>>("job:*")
        .await;

    let job_count = job_keys.map(|k| k.len() as u64).unwrap_or(0);

    let ping: Result<String, _> = redis::cmd("PING")
        .query_async(&mut state.redis.clone())
        .await;

    let stats = CacheStats {
        translation_count,
        job_count,
        redis_connected: ping.is_ok(),
    };

    (StatusCode::OK, Json(stats))
}

pub async fn clear_cache(State(state): State<SharedState>) -> impl IntoResponse {
    let keys: Result<Vec<String>, _> = state
        .redis
        .clone()
        .keys::<_, Vec<String>>("translation:*")
        .await;

    if let Ok(keys) = keys {
        if !keys.is_empty() {
            let _: Result<(), _> = state.redis.clone().del::<_, ()>(keys).await;
        }
    }

    StatusCode::NO_CONTENT
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/stats", get(get_stats))
        .route("/stats/cache", axum::routing::delete(clear_cache))
}
```

### Update Routes Module

```rust
// File: translator-server/src/routes/mod.rs

pub mod health;
pub mod jobs;
pub mod stats;
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
        .merge(stats::router())
        .layer(cors)
        .with_state(state)
}
```

### Verification: Section 3.4

```bash
cargo run -p translator-server &

curl -s http://localhost:3000/stats | jq

# Submit a translation
curl -s -X POST http://localhost:3000/translate \
  -H "Content-Type: application/json" \
  -d '{"texts":["Hello"],"src_lang":"eng_Latn","targets":{"fr":"fra_Latn"}}'

sleep 1
curl -s http://localhost:3000/stats | jq

kill %1
```

**Expected output**:

```json
{
  "translation_count": 0,
  "job_count": 1,
  "redis_connected": true
}
```

---

## Chapter Summary

You now have Redis integrated for:

| Feature | Implementation |
|---------|----------------|
| Connection pooling | Multiplexed async connection |
| Translation cache | SHA256-keyed with configurable TTL |
| Batch operations | MGET/pipeline for efficiency |
| Job storage | Persistent job metadata |
| Statistics | Monitor cache and job counts |

### API Summary (Updated)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| POST | `/translate` | Submit translation job |
| GET | `/jobs/{id}` | Get job status |
| DELETE | `/jobs/{id}` | Remove job |
| GET | `/stats` | Cache statistics |
| DELETE | `/stats/cache` | Clear translation cache |

### Redis Key Schema

| Pattern | Purpose | TTL |
|---------|---------|-----|
| `translation:{hash}:{src}:{tgt}` | Cached translation | 24 hours |
| `job:{uuid}` | Job metadata | 24 hours |

---

## Next Chapter

Continue to [Chapter 4: Building the Web UI](./04-web-ui.md) to:

- Add Maud for type-safe HTML templates
- Build Web Components for reusable UI elements
- Create job status polling with vanilla JavaScript
