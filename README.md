# Rust CSV Translation Pipeline

An async Rust pipeline for generating multilingual CSVs using pluggable translation providers (e.g. NLLB today, Gemini in the future).  
Designed as a **background service** to power spreadsheet-style localization workflows.

---

## Overview

This project provides a reusable Rust crate and CLI tool that:

- Ingests a CSV file containing source text  
- Translates that text into multiple target languages  
- Emits a new CSV with one column per target language  
- Supports machine translation (NLLB) today and is extensible to future providers (e.g. Gemini, human translation)

The system is designed to be:
- **Async** (Tokio)  
- **Provider-agnostic**  
- **Config-driven**  
- **Efficient at scale** (batched translation)  
- **Robust** (timeouts, retries, backoff)

---

## Architecture

'''CSV
 ↓
Rust async pipeline (batching, retries, CSV IO)
 ↓
Translation Provider (HTTP)
 ↓
CSV with language columns

### Key design principles
- **Separation of concerns**  
  The Rust crate owns orchestration and IO. Model serving lives behind an HTTP boundary.
- **Pluggable providers**  
  Translation providers implement a common interface (`TranslationProvider`).
- **Batch-oriented**  
  Designed for background jobs, not interactive latency.

---

## Supported Providers

### Mock Provider
- Deterministic, offline testing  
- No network calls  
- Useful for CI and development  

### NLLB REST Provider
- Calls an HTTP service implementing:

'''POST /translate  
{ src_lang, tgt_lang, text[] } → { translations[] }'''

- Includes:
  - request + connect timeouts  
  - retry with exponential backoff + jitter  
  - batch translation support  

### Gemini (stub)
- CLI-selectable but not implemented  
- Interface exists to demonstrate forward compatibility  

---

## CLI Usage

The primary entrypoint is a CLI binary:

'''cargo run --bin translate_csv -- [OPTIONS]'''

### Example (NLLB)

'''cargo run --bin translate_csv -- \
  --provider nllb \
  --nllb-base-url http://localhost:8080 \
  --input input.csv \
  --output output.csv \
  --targets-file targets_50.json'''

### Example (Mock)

'''cargo run --bin translate_csv -- \
  --provider mock \
  --input input.csv \
  --output output.csv'''

---

## CSV Format

### Input CSV
'''source
Book a ride
Your driver has arrived'''

### Output CSV
'''source,fr,es,de
Book a ride,Réservez un trajet,Reserve un viaje,Buchen Sie eine Fahrt
Your driver has arrived,Votre chauffeur est arrivé,Su conductor ha llegado,Ihr Fahrer ist angekommen'''

- Input columns are preserved  
- One column is added per target language  
- Row order is preserved  

---

## Target Language Configuration

Target languages are **data-driven**, not hardcoded.

### targets_50.json
'''[
  { "col": "fr", "lang": "fra_Latn" },
  { "col": "es", "lang": "spa_Latn" },
  { "col": "de", "lang": "deu_Latn" }
]'''

This allows scaling to **50+ languages** without code changes.

---

## Performance Characteristics

### Batching
- Source rows are translated in configurable batches (default: 128)
- Reduces HTTP calls from:

'''rows × languages'''

to:

'''ceil(rows / batch_size) × languages'''

This provides **orders-of-magnitude throughput improvements** for large CSVs.

---

### Concurrency
- Concurrency is applied at the provider level  
- Designed to work with horizontally scaled model servers  

---

### Known Bottleneck
- End-to-end runtime is dominated by **model inference**, not Rust orchestration  
- Local CPU-based NLLB serving will be slow relative to GPU or optimized runtimes  

---

## Reliability Features

The NLLB provider includes:
- Request timeout  
- Connect timeout  
- Retry with exponential backoff + jitter  
- Retryable error classification (timeouts, 429, 5xx)  
- Fail-fast behavior on non-retryable errors  

This makes the pipeline suitable for **unattended background execution**.

---

## Local NLLB Server (Development)

For local testing, NLLB can be run via a simple **FastAPI + HuggingFace** server (e.g. Dockerized).  
The Rust pipeline treats the model as an external service and does not depend on Python directly.

In production, this provider would point to an internal model-serving endpoint.

---

## Project Structure

'''src/
  lib.rs                # Core translation engine
  csv_pipeline.rs       # CSV orchestration + batching
  provider/
    mock.rs
    nllb_rest.rs
  error.rs

src/bin/
  translate_csv.rs      # CLI tool

examples/
  translate_csv.rs      # Minimal usage example'''

---

## Non-Goals

- No UI implementation (intended to back spreadsheet clients)  
- No direct model inference in Rust  
- No streaming/interactive output (background batch job by design)  

---

## Future Extensions

- Gemini provider implementation  
- Human translation provider (CSV / DB backed)  
- Parallelization across languages  
- Streaming output for very large CSVs  
- Metrics and structured logging  
- GPU-optimized model serving (e.g. CTranslate2)  

---

## Summary

This project delivers a **production-shaped, async Rust translation pipeline** that meets the original requirements while remaining extensible and efficient. It is designed to integrate cleanly into larger localization workflows and internal tooling ecosystems.
