# CSV Translation Pipeline

A production-ready async Rust library for translating CSV files using pluggable translation providers.

## Quick Start (30 seconds)

```bash
# Clone and build
git clone https://github.com/yourorg/csv-translation-pipeline.git
cd csv-translation-pipeline
cargo build --release

# Translate a CSV (mock provider for testing)
echo "source\nHello\nGoodbye" > input.csv
cargo run --bin translate_csv -- --input input.csv --output output.csv

# View results
cat output.csv
# source,fr,es
# Hello,[mock::fra_Latn] Hello,[mock::spa_Latn] Hello
# Goodbye,[mock::fra_Latn] Goodbye,[mock::spa_Latn] Goodbye
```

## Installation

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
translator = { git = "https://github.com/yourorg/csv-translation-pipeline.git" }
```

### As a CLI Tool

```bash
cargo install --path .
```

## Usage

### Library API

```rust
use translator::{translate_csv, CsvTranslateConfig, provider::mock::MockProvider};

#[tokio::main]
async fn main() -> Result<(), translator::TranslateError> {
    let config = CsvTranslateConfig {
        source_col: "text".into(),
        src_lang: "eng_Latn".into(),
        targets: vec![
            ("fr".into(), "fra_Latn".into()),
            ("es".into(), "spa_Latn".into()),
        ],
    };

    translate_csv(&MockProvider, "input.csv", "output.csv", config).await
}
```

### Job API (for Web Services)

```rust
use translator::{JobManager, TranslationRequest, JobStatus};
use translator::provider::nllb_rest::NllbRestProvider;
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Create a job manager (share across your application)
    let manager = JobManager::new(NllbRestProvider::new("http://localhost:8080"));

    // Submit a translation job (returns immediately)
    let job_id = manager.submit(TranslationRequest {
        texts: vec!["Hello".into(), "World".into()],
        src_lang: "eng_Latn".into(),
        targets: vec![("fr".into(), "fra_Latn".into())],
    });

    // Poll for completion
    loop {
        match manager.get(&job_id) {
            Some(JobStatus::Pending) => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Some(JobStatus::Complete { translations }) => {
                println!("French: {:?}", translations.get("fr"));
                break;
            }
            Some(JobStatus::Failed { error }) => {
                eprintln!("Translation failed: {}", error);
                break;
            }
            None => panic!("Job disappeared"),
        }
    }
}
```

### CLI

```bash
# Using mock provider (for testing)
translate_csv --input data.csv --output translated.csv

# Using NLLB REST provider (self-hosted)
translate_csv \
    --provider nllb \
    --nllb-url http://localhost:8080 \
    --input data.csv \
    --output translated.csv \
    --source-col text \
    --src-lang eng_Latn \
    --target fr=fra_Latn \
    --target es=spa_Latn \
    --target de=deu_Latn

# Using Gemini provider (cloud LLM)
export GEMINI_API_KEY="your-api-key-from-aistudio.google.com"
translate_csv \
    --provider gemini \
    --input data.csv \
    --output translated.csv \
    --target fr=fra_Latn
```

## Architecture

```
                    +------------------+
                    |   Entry Points   |
                    +------------------+
                           |
         +-----------------+-----------------+
         |                 |                 |
    +----v----+      +-----v-----+     +-----v-----+
    |   CLI   |      | Library   |     |  Job API  |
    +---------+      +-----------+     +-----------+
         |                 |                 |
         +-----------------+-----------------+
                           |
                    +------v------+
                    | CSV Pipeline |
                    +-------------+
                           |
                    +------v------+
                    |  Provider   |  <-- TranslationProvider trait
                    |   Trait     |
                    +-------------+
                           |
    +----------+-----------+-----------+-----------+
    |          |           |           |
+---v---+ +----v----+ +----v----+ +----v----+
| Mock  | |  NLLB   | | Marian  | | Gemini  |
|Provider| |  REST   | |(rust-  | | (Cloud  |
|       | |         | | bert)  | |  LLM)   |
+-------+ +---------+ +---------+ +---------+
```

### Key Design Decisions

1. **Trait-based Providers**: All translation backends implement `TranslationProvider`, making it trivial to add new providers (Gemini, OpenAI, human translation, etc.)

2. **Async Throughout**: Built on Tokio for non-blocking I/O. Perfect for web services.

3. **Batch-Oriented**: Designed for background jobs, not interactive latency. Translates all texts for one language before moving to the next.

4. **Resilient**: NLLB provider includes retry with exponential backoff, jitter, and timeout handling.

## Available Providers

| Provider | Use Case | Requirements |
|----------|----------|--------------|
| `MockProvider` | Testing, development | None |
| `NllbRestProvider` | Production (200+ languages) | NLLB REST server |
| `MarianProvider` | Offline, pure Rust (50+ language pairs) | libtorch |
| `GeminiProvider` | Cloud LLM (context-aware, 100+ languages) | API key |

### Choosing a Provider

- **MockProvider**: Use for testing and development
- **NllbRestProvider**: Best for self-hosted deployments with many languages
- **MarianProvider**: Best for offline/edge deployments (pure Rust)
- **GeminiProvider**: Best for cloud deployments without ML infrastructure

## Adding a New Provider

1. Create `src/provider/your_provider.rs`:

```rust
use crate::error::TranslateError;
use crate::provider::TranslationProvider;

pub struct YourProvider { /* config */ }

impl YourProvider {
    pub fn new(/* config */) -> Self { /* ... */ }
}

impl TranslationProvider for YourProvider {
    async fn translate_batch(
        &self,
        src_lang: &str,
        tgt_lang: &str,
        texts: &[String],
    ) -> Result<Vec<String>, TranslateError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        // Your translation logic here
        // MUST return Vec with same length as input
        todo!()
    }

    fn name(&self) -> &'static str {
        "your-provider"
    }
}
```

2. Export from `src/provider/mod.rs`:
```rust
pub mod your_provider;
```

3. (Optional) Add CLI support in `src/bin/translate_csv.rs`

## CSV Format

### Input
```csv
source
Hello
Goodbye
```

### Output (with French and Spanish targets)
```csv
source,fr,es
Hello,[translation],[translation]
Goodbye,[translation],[translation]
```

## Requirements

- **Rust 1.75+** (for async fn in traits)
- **Tokio runtime** (provided by this crate)
- **libtorch** (optional, for MarianMT provider only)

## Local Development Setup

### Without MarianMT (Recommended for Quick Start)

No external dependencies required:

```bash
cargo build --release
./target/release/translate_csv --provider mock --input data.csv --output out.csv
```

### With MarianMT (Pure Rust Translation)

MarianMT requires **libtorch** - standalone C++ libraries. **No Python needed.**

#### macOS

```bash
# 1. Download libtorch directly (no Python/PyTorch needed)
curl -LO https://download.pytorch.org/libtorch/cpu/libtorch-macos-arm64-2.4.0.zip
unzip libtorch-macos-arm64-2.4.0.zip

# 2. Set environment variables (add to ~/.zshrc)
export LIBTORCH=$(pwd)/libtorch
export DYLD_LIBRARY_PATH=${LIBTORCH}/lib:$DYLD_LIBRARY_PATH

# 3. Build with MarianMT support
cargo build --features marian

# 4. Run tests (downloads ~300MB model on first run)
cargo test --features marian marian -- --ignored --nocapture
```

#### Linux

```bash
# 1. Download libtorch (CPU version, ~200MB)
wget https://download.pytorch.org/libtorch/cpu/libtorch-cxx11-abi-shared-with-deps-2.4.0%2Bcpu.zip
unzip libtorch-cxx11-abi-shared-with-deps-2.4.0+cpu.zip

# 2. Set environment variables (add to ~/.bashrc)
export LIBTORCH=$(pwd)/libtorch
export LD_LIBRARY_PATH=${LIBTORCH}/lib:$LD_LIBRARY_PATH

# 3. Build
cargo build --features marian
```

## Deployment

### Render.com (Free Tier)

Deploy to Render.com for free hosting:

1. **Create `Dockerfile`** in project root (see below)
2. **Create `render.yaml`** for infrastructure-as-code
3. **Push to GitHub** and connect to Render

Create `Dockerfile`:

```dockerfile
FROM rust:1.83-bookworm AS builder
WORKDIR /app

# Install libtorch (standalone C++ libs, no Python)
RUN apt-get update && apt-get install -y wget unzip && rm -rf /var/lib/apt/lists/*
RUN wget -q https://download.pytorch.org/libtorch/cpu/libtorch-cxx11-abi-shared-with-deps-2.4.0%2Bcpu.zip \
    && unzip -q libtorch-cxx11-abi-shared-with-deps-2.4.0+cpu.zip -d /opt \
    && rm *.zip
ENV LIBTORCH=/opt/libtorch
ENV LD_LIBRARY_PATH=${LIBTORCH}/lib:${LD_LIBRARY_PATH}

COPY . .
RUN cargo build --release --features marian

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libgomp1 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /opt/libtorch/lib/*.so* /usr/local/lib/
RUN ldconfig
COPY --from=builder /app/target/release/translate_csv /usr/local/bin/
ENV HF_HOME=/app/.cache
EXPOSE 8080
CMD ["translate_csv", "--help"]
```

Create `render.yaml`:

```yaml
services:
  - type: web
    name: csv-translation-pipeline
    runtime: docker
    plan: free
    healthCheckPath: /health
    envVars:
      - key: HF_HOME
        value: /app/.cache
```

See [architecture.md](./architecture.md#deployment) for detailed deployment documentation.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime |
| `csv-async` | Non-blocking CSV I/O |
| `reqwest` | HTTP client for REST providers |
| `serde` | Serialization |
| `thiserror` | Error handling |
| `clap` | CLI argument parsing |
| `uuid` | Job ID generation |
| `fastrand` | Random jitter for retries |

## Future Extensions

- Redis Translation Cache (avoid duplicate ML calls)
- Redis Job Storage (replace in-memory HashMap)
- OpenAI provider
- Distributed workers via Redis queue
- Metrics and tracing

## License

MIT

---

For detailed architecture documentation, see [architecture.md](./architecture.md).
