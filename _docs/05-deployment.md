# Chapter 5: Deploying to Render.com

**Duration**: 2 hours
**Sections**: 4 (30 minutes each)

## Overview

In this chapter, you package the application for production and deploy to Render.com.

By the end, you will have:

- A multi-stage Dockerfile
- Infrastructure-as-code with `render.yaml`
- A live HTTPS service
- Maintenance scripts

---

## Prerequisites

- Completed Chapter 4
- GitHub account
- Render.com account (free tier available)
- Docker installed locally

---

## Section 5.1: Creating the Dockerfile (30 min)

### Learning Objective

Create an optimized multi-stage Dockerfile.

### Create Dockerfile

```dockerfile
# File: Dockerfile

# Stage 1: Build
FROM rust:1.83-bookworm AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY translator ./translator
COPY translator-server ./translator-server

RUN cargo build --release -p translator-server

# Stage 2: Runtime
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/translator-server /usr/local/bin/

RUN useradd -m -u 1000 translator
USER translator

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

CMD ["translator-server"]
```

### Create .dockerignore

```
# File: .dockerignore

target/
.git/
_docs/
*.md
.env
tests/
examples/
Dockerfile*
docker-compose*
```

### Create docker-compose.yml

```yaml
# File: docker-compose.yml

version: "3.8"

services:
  app:
    build: .
    ports:
      - "3000:3000"
    environment:
      - REDIS_URL=redis://redis:6379
      - PORT=3000
      - RUST_LOG=info,translator_server=debug
    depends_on:
      - redis

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
```

### Verification: Section 5.1

```bash
docker build -t translator-server:local .
```

**Expected**: Build completes successfully

```bash
docker-compose up -d
curl http://localhost:3000/health
docker-compose down
```

**Expected**: `{"status":"healthy","version":"0.1.0"}`

---

## Section 5.2: Render.com Configuration (30 min)

### Learning Objective

Create infrastructure-as-code for Render.com.

### Create render.yaml

```yaml
# File: render.yaml

services:
  - type: web
    name: translator-server
    runtime: docker
    dockerfilePath: ./Dockerfile
    plan: free
    region: oregon
    healthCheckPath: /health
    autoDeploy: true
    envVars:
      - key: REDIS_URL
        fromService:
          name: redis
          type: redis
          property: connectionString
      - key: PORT
        value: "3000"
      - key: RUST_LOG
        value: info,translator_server=debug
      - key: CACHE_TTL_SECONDS
        value: "86400"

  - type: redis
    name: redis
    plan: free
    region: oregon
    maxmemoryPolicy: allkeys-lru
```

### Render.com Free Tier Limits

| Resource | Limit |
|----------|-------|
| Web Services | Spin down after 15 min idle |
| Redis Memory | 25 MB |
| Build Time | 30 min max |
| Bandwidth | 100 GB/month |

### Manual Setup Alternative

If you prefer the dashboard:

1. Create Redis: New > Redis > Free plan
2. Create Web Service: New > Web Service > Connect repo
3. Add environment variables:
   - `PORT`: 3000
   - `REDIS_URL`: (from Redis instance)
   - `RUST_LOG`: info,translator_server=debug

### Verification: Section 5.2

```bash
python3 -c "import yaml; yaml.safe_load(open('render.yaml'))"
```

**Expected**: No errors (valid YAML)

---

## Section 5.3: Production Configuration (30 min)

### Learning Objective

Add production-ready configuration.

### Update Config

```rust
// File: translator-server/src/config.rs

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub redis_url: String,
    pub port: u16,
    pub cache_ttl_seconds: u64,
    pub environment: String,
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
                .unwrap_or(86400),
            environment: env::var("RENDER_SERVICE_NAME")
                .map(|_| "production".into())
                .unwrap_or_else(|_| "development".into()),
        }
    }

    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }
}
```

### Update Health Check

```rust
// File: translator-server/src/routes/health.rs

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;
use crate::state::SharedState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub environment: String,
    pub redis_connected: bool,
}

pub async fn health_check(State(state): State<SharedState>) -> impl IntoResponse {
    let ping: Result<String, _> = redis::cmd("PING")
        .query_async(&mut state.redis.clone())
        .await;

    let redis_connected = ping.is_ok();

    let response = HealthResponse {
        status: if redis_connected { "healthy" } else { "degraded" },
        version: env!("CARGO_PKG_VERSION"),
        environment: state.config.environment.clone(),
        redis_connected,
    };

    let status = if redis_connected {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status, Json(response))
}

pub fn router() -> Router<SharedState> {
    Router::new().route("/health", get(health_check))
}
```

### Verification: Section 5.3

```bash
docker-compose up --build -d
curl -i http://localhost:3000/health
docker-compose down
```

**Expected**: Response includes `"environment": "development"`

---

## Section 5.4: Deploying to Production (30 min)

### Learning Objective

Deploy the service to Render.com.

### Push to GitHub

```bash
git add .
git commit -m "Prepare for Render.com deployment"
git push origin main
```

### Deploy via Blueprint

1. Go to [dashboard.render.com](https://dashboard.render.com)
2. Click New > Blueprint
3. Connect your GitHub repository
4. Render detects `render.yaml`
5. Review services: `translator-server` + `redis`
6. Click Apply

### Monitor Deployment

1. Watch build logs (5-10 minutes first build)
2. Find your URL: `https://translator-server-xxxx.onrender.com`

### Verify Deployment

```bash
export RENDER_URL="https://translator-server-xxxx.onrender.com"

# Health check
curl "$RENDER_URL/health"

# Test translation
curl -X POST "$RENDER_URL/translate" \
  -H "Content-Type: application/json" \
  -d '{"texts":["Hello"],"src_lang":"eng_Latn","targets":{"fr":"fra_Latn"}}'

# Open UI
open "$RENDER_URL"
```

### Troubleshooting

| Issue | Solution |
|-------|----------|
| Build fails | Check Dockerfile syntax, verify workspace members |
| Service crashes | Check REDIS_URL, view logs in dashboard |
| Health check fails | Verify PORT matches Render config |
| Slow first request | Free tier spins down; ~30s cold start |

### Verification: Section 5.4

```bash
curl https://your-service.onrender.com/health
```

**Expected**:

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "environment": "production",
  "redis_connected": true
}
```

---

## Chapter Summary

You now have a production deployment:

| Component | Description |
|-----------|-------------|
| Dockerfile | Multi-stage build, minimal runtime |
| render.yaml | Infrastructure-as-code |
| Health check | Enhanced with Redis status |
| HTTPS | Automatic via Render.com |

### Deployment Architecture

```
GitHub Push
    |
    v
Render.com (auto-deploy)
    |
    +---> Build Docker Image
    |
    +---> Deploy Web Service
    |         |
    |         v
    |     translator-server ---> Redis
    |
    +---> Health Check Passes
    |
    v
Live at https://xxx.onrender.com
```

### Files Created

| File | Purpose |
|------|---------|
| `Dockerfile` | Multi-stage build |
| `.dockerignore` | Build optimization |
| `docker-compose.yml` | Local testing |
| `render.yaml` | Render.com config |

### Cost (Free Tier)

| Resource | Cost |
|----------|------|
| Web Service | $0 (spins down when idle) |
| Redis | $0 (25 MB limit) |

For always-on: Upgrade to Starter ($7/month).

---

## Tutorial Complete

You built a production translation service:

1. **Chapter 1**: Understood the `translator` library
2. **Chapter 2**: Created Axum web server
3. **Chapter 3**: Added Redis caching
4. **Chapter 4**: Built Web Components UI
5. **Chapter 5**: Deployed to Render.com

### What You Built

| Component | Technology |
|-----------|------------|
| Library | Rust async CSV translation |
| Server | Axum HTTP API |
| Cache | Redis with TTL |
| UI | Maud + Web Components |
| Infrastructure | Docker + Render.com |

### Possible Extensions

- Replace MockProvider with NLLB or MarianMT
- Add authentication (API keys, OAuth)
- Add rate limiting
- Add Prometheus metrics
- Add CSV file upload
- Add webhooks for job completion

Thank you for following this tutorial.
