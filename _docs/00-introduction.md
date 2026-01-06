# Introduction: Building a Production Translation Service

**Total Duration**: 10-12 hours across 5 chapters

## What You Will Build

This tutorial transforms the `translator` library into a production-ready web service:

| Component | Purpose |
|-----------|---------|
| Axum HTTP API | Submit translation jobs, poll for results |
| Redis caching | Avoid redundant ML model calls |
| Redis job storage | Persist jobs across restarts |
| Web UI | Vanilla JavaScript + Web Components interface |
| Docker deployment | Deploy to Render.com |

By the end, you will have a translation service that scales horizontally and provides real-time feedback via polling.

---

## Prerequisites

### Required Software

| Tool | Version | Verification |
|------|---------|--------------|
| Rust | 1.75+ | `rustc --version` |
| Cargo | Latest | `cargo --version` |
| Docker | 20.10+ | `docker --version` |
| Redis | 7.0+ | `redis-server --version` |
| Git | 2.0+ | `git --version` |

### Required Knowledge

- Reading and writing Rust
- Async/await basics
- HTTP API concepts (REST)
- Basic HTML

### Verification: Setup Complete

Clone and verify the existing crate works:

```bash
git clone https://github.com/yourorg/csv-translation-pipeline.git
cd csv-translation-pipeline

# Run tests
cargo test

# Test the CLI
echo "source
Hello
Goodbye" > /tmp/test.csv
cargo run --bin translate_csv -- --input /tmp/test.csv --output /tmp/out.csv
cat /tmp/out.csv
```

**Expected output**:

```csv
source,fr,es
Hello,[mock::fra_Latn] Hello,[mock::spa_Latn] Hello
Goodbye,[mock::fra_Latn] Goodbye,[mock::spa_Latn] Goodbye
```

If you see this output, your setup is complete.

---

## Tutorial Structure

| Chapter | Duration | What You Build |
|---------|----------|----------------|
| 1. Understanding the Crate | 2 hours | Deep knowledge of the library architecture |
| 2. Creating the Axum Workspace | 2 hours | HTTP server with health, translate, and job endpoints |
| 3. Adding Redis | 2 hours | Translation caching and persistent job storage |
| 4. Building the Web UI | 2 hours | Maud templates + Web Components interface |
| 5. Deploying to Render.com | 2 hours | Production Docker deployment |

Each chapter contains 4 sections of approximately 30 minutes each.

---

## Architecture Overview

```
                        +------------------+
                        |   Web Frontend   |
                        | (Maud + Web Comp)|
                        +--------+---------+
                                 |
                                 v
+---------------------+   +---------+   +------------------+
|    Render.com       |   |  Axum   |   |     Redis        |
| (Docker Container)  |<->| Server  |<->| (Cache + Jobs)   |
+---------------------+   +---------+   +------------------+
                                 |
                                 v
                        +------------------+
                        |    translator    |
                        |    (library)     |
                        +------------------+
                                 |
        +------------+-----------+------------+
        |            |                        |
   +----v----+  +----v----+              +----v----+
   |  Mock   |  |  NLLB   |              | Marian  |
   | Provider|  |  REST   |              |(libtorch|
   |(testing)|  |(server) |              |  Rust)  |
   +---------+  +---------+              +---------+
```

### Translation Providers

| Provider | Use Case | Requirements |
|----------|----------|--------------|
| MockProvider | Testing, development | None |
| NllbRestProvider | Production (200+ languages) | NLLB server running |
| MarianProvider | Offline, pure Rust | libtorch + `marian` feature |

---

## Key Design Principles

### 1. Incremental Progress

Each section builds on the previous. You never need to refactor earlier code.

### 2. Verifiable Feedback

Every section ends with a concrete verification step:

- Unit tests you can run
- `curl` commands with expected output
- Visual checks in the browser
- Redis CLI inspection commands

### 3. Production Patterns

The code uses battle-tested patterns:

- Error handling with `thiserror`
- Structured logging with `tracing`
- Graceful shutdown handling
- Health checks for load balancers

### 4. Minimal UI

The Web Components interface is intentionally simple. It verifies the backend works. Enhance it later if needed.

---

## File Structure After Completion

```
csv-translation-pipeline/
  Cargo.toml                 # Workspace root
  Cargo.lock
  Dockerfile
  render.yaml
  translator/                # Original library crate
    Cargo.toml
    src/
      lib.rs
      jobs.rs
      csv_pipeline.rs
      error.rs
      provider/
        mod.rs
        mock.rs
        nllb_rest.rs
        marian.rs
  translator-server/         # New binary crate
    Cargo.toml
    src/
      main.rs
      routes/
        mod.rs
        health.rs
        translate.rs
        jobs.rs
      state.rs
      templates/
        mod.rs
        layout.rs
        translate_form.rs
        job_status.rs
      cache.rs
      config.rs
  _docs/                     # This tutorial
    00-introduction.md
    01-understanding-the-crate.md
    02-axum-workspace.md
    03-redis-integration.md
    04-web-ui.md
    05-deployment.md
```

---

## Conventions

### Code Blocks

Every code block shows which file it belongs to:

```rust
// File: src/example.rs

pub fn example() {
    // Implementation
}
```

### Verification Blocks

Each section ends with a verification block:

```
Verification: Section N.M
=========================
Run: cargo test -p translator-server
Expected: All tests pass

Run: curl http://localhost:3000/health
Expected: {"status":"healthy"}
```

### Time Estimates

Times are approximate for a developer familiar with Rust but new to these libraries:

- Section: ~30 minutes
- Chapter: ~2 hours

---

## Troubleshooting

If you get stuck:

1. Verify the previous section's verification step passed
2. Re-read the current section for common issues
3. Check the Git commit history for that chapter
4. Open an issue on the repository

---

## Ready to Begin

Continue to [Chapter 1: Understanding the Existing Crate](./01-understanding-the-crate.md).

The journey from library to production service begins with understanding what already exists.
