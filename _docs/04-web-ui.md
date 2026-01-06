# Chapter 4: Building the Web UI

**Duration**: 2 hours
**Sections**: 4 (30 minutes each)

## Overview

In this chapter, you build a web interface using:

- **Maud templates** for server-side HTML
- **Vanilla JavaScript** with Web Components
- **Plain CSS** with custom properties

By the end, you will have:

- A translation form with provider selection
- Real-time job status updates via polling
- A statistics page showing cache metrics

---

## Prerequisites

- Completed Chapter 3
- Redis running for job storage

---

## Section 4.1: Adding Maud Templates (30 min)

### Learning Objective

Set up Maud for type-safe HTML generation.

### Add Maud Dependency

```toml
# File: Cargo.toml (add to [workspace.dependencies])

maud = { version = "0.26", features = ["axum"] }
```

```toml
# File: translator-server/Cargo.toml (add to [dependencies])

maud.workspace = true
```

### Create Templates Module

```bash
mkdir -p translator-server/src/templates
```

```rust
// File: translator-server/src/templates/mod.rs

pub mod layout;
pub mod components;
pub mod pages;
```

### Create Base Layout

```rust
// File: translator-server/src/templates/layout.rs

use maud::{html, Markup, DOCTYPE};

pub fn base(title: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " - Translation Service" }
                style { (STYLES) }
            }
            body {
                header {
                    h1 { "Translation Service" }
                    nav {
                        a href="/" { "Home" }
                        " | "
                        a href="/ui/stats" { "Stats" }
                    }
                }
                main { (content) }
                footer {
                    p { "Built with Rust, Axum, and Web Components" }
                }
                script type="module" { (COMPONENTS_JS) }
            }
        }
    }
}

const STYLES: &str = r#"
:root {
    --color-primary: #0066cc;
    --color-success: #006600;
    --color-warning: #996600;
    --color-error: #660000;
    --spacing-md: 1rem;
    --radius: 4px;
}

* { box-sizing: border-box; }

body {
    font-family: system-ui, sans-serif;
    max-width: 800px;
    margin: 0 auto;
    padding: 1rem;
    line-height: 1.6;
}

header { border-bottom: 2px solid #333; padding-bottom: 1rem; margin-bottom: 2rem; }
nav a { color: var(--color-primary); }

form {
    background: #fff;
    padding: 1.5rem;
    border-radius: 8px;
    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
}

label { display: block; margin-bottom: 0.5rem; font-weight: 600; }
textarea, input, select {
    width: 100%;
    padding: 0.75rem;
    border: 1px solid #ddd;
    border-radius: var(--radius);
    margin-bottom: 1rem;
}
textarea { min-height: 120px; resize: vertical; }

button {
    background: var(--color-primary);
    color: white;
    border: none;
    padding: 0.75rem 1.5rem;
    border-radius: var(--radius);
    cursor: pointer;
}
button:hover:not(:disabled) { background: #0052a3; }
button:disabled { background: #ccc; cursor: not-allowed; }

.form-row { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }

.status-pending { color: var(--color-warning); background: #fff3cd; padding: 0.5rem 1rem; border-radius: var(--radius); }
.status-complete { color: var(--color-success); background: #d4edda; padding: 0.5rem 1rem; border-radius: var(--radius); }
.status-error { color: var(--color-error); background: #f8d7da; padding: 0.5rem 1rem; border-radius: var(--radius); }

.translation-result { background: #fff; padding: 1rem; border-radius: 8px; margin-top: 1rem; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
.translation-item { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; padding: 0.5rem 0; border-bottom: 1px solid #eee; }

.spinner {
    display: inline-block;
    width: 1rem; height: 1rem;
    border: 2px solid #ddd;
    border-top-color: var(--color-primary);
    border-radius: 50%;
    animation: spin 1s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.btn-secondary { background: #6c757d; padding: 0.25rem 0.5rem; font-size: 0.875rem; }
"#;

const COMPONENTS_JS: &str = r##"
// TranslationForm: Handles form submission and polling
class TranslationForm extends HTMLFormElement {
    #pollInterval = null;

    connectedCallback() {
        this.addEventListener('submit', this);
    }

    disconnectedCallback() {
        this.removeEventListener('submit', this);
        if (this.#pollInterval) clearInterval(this.#pollInterval);
    }

    async handleEvent(e) {
        if (e.type === 'submit') {
            e.preventDefault();
            await this.#handleSubmit();
        }
    }

    async #handleSubmit() {
        const formData = new FormData(this);
        const texts = formData.get('texts').split('\n').map(l => l.trim()).filter(l => l);

        if (texts.length === 0) {
            this.#showStatus('error', { message: 'Enter at least one line' });
            return;
        }

        this.#setSubmitDisabled(true);
        this.#showStatus('pending', { message: 'Submitting...' });

        try {
            const response = await fetch('/translate', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    texts,
                    src_lang: formData.get('src_lang'),
                    targets: { result: formData.get('tgt_lang') }
                })
            });

            if (!response.ok) throw new Error((await response.json()).error);

            const { job_id } = await response.json();
            this.#showStatus('pending', { message: 'Translating...', jobId: job_id });
            this.#startPolling(job_id, texts);
        } catch (error) {
            this.#showStatus('error', { message: error.message });
            this.#setSubmitDisabled(false);
        }
    }

    #startPolling(jobId, sourceTexts) {
        let attempts = 0;
        this.#pollInterval = setInterval(async () => {
            if (++attempts > 120) {
                clearInterval(this.#pollInterval);
                this.#showStatus('error', { message: 'Timeout' });
                this.#setSubmitDisabled(false);
                return;
            }

            try {
                const status = await (await fetch(`/jobs/${jobId}`)).json();
                if (status.status === 'complete') {
                    clearInterval(this.#pollInterval);
                    this.#showStatus('complete', { sourceTexts, translations: status.translations });
                    this.#setSubmitDisabled(false);
                } else if (status.status === 'failed') {
                    clearInterval(this.#pollInterval);
                    this.#showStatus('error', { message: status.error });
                    this.#setSubmitDisabled(false);
                }
            } catch (e) { /* retry */ }
        }, 500);
    }

    #showStatus(status, detail) {
        this.dispatchEvent(new CustomEvent('translation-status', { bubbles: true, detail: { status, ...detail } }));
    }

    #setSubmitDisabled(disabled) {
        for (const el of this.elements) if (el.type === 'submit') el.disabled = disabled;
    }
}
customElements.define('translation-form', TranslationForm, { extends: 'form' });

// TranslationResult: Displays status and results
class TranslationResult extends HTMLElement {
    connectedCallback() {
        document.addEventListener('translation-status', this);
    }

    disconnectedCallback() {
        document.removeEventListener('translation-status', this);
    }

    handleEvent(e) {
        if (e.type === 'translation-status') {
            this.#render(e.detail);
        }
    }

    #render(detail) {
        this.innerHTML = '';
        const { status } = detail;

        if (status === 'pending') {
            this.innerHTML = `<div class="status-pending"><span class="spinner"></span> ${detail.message || 'Processing...'}</div>`;
        } else if (status === 'complete') {
            let html = '<div class="status-complete">Complete!</div>';
            for (const [lang, texts] of Object.entries(detail.translations)) {
                html += `<div class="translation-result"><h3>Translated: ${lang}</h3>`;
                detail.sourceTexts.forEach((src, i) => {
                    html += `<div class="translation-item"><div><strong>Original:</strong> ${src}</div><div><strong>Translated:</strong> ${texts[i]}</div></div>`;
                });
                html += '</div>';
            }
            this.innerHTML = html;
        } else if (status === 'error') {
            this.innerHTML = `<div class="status-error"><strong>Error:</strong> ${detail.message}</div>`;
        }
    }
}
customElements.define('translation-result', TranslationResult);
"##;
```

### Create Components

```rust
// File: translator-server/src/templates/components.rs

use maud::{html, Markup};

pub fn translation_form() -> Markup {
    html! {
        form is="translation-form" {
            div {
                label for="texts" { "Text to translate (one per line):" }
                textarea id="texts" name="texts" placeholder="Hello\nGoodbye" required {}
            }

            div class="form-row" {
                div {
                    label for="src_lang" { "Source:" }
                    select id="src_lang" name="src_lang" {
                        option value="eng_Latn" selected { "English" }
                        option value="fra_Latn" { "French" }
                        option value="deu_Latn" { "German" }
                        option value="spa_Latn" { "Spanish" }
                    }
                }
                div {
                    label for="tgt_lang" { "Target:" }
                    select id="tgt_lang" name="tgt_lang" {
                        option value="fra_Latn" selected { "French" }
                        option value="eng_Latn" { "English" }
                        option value="deu_Latn" { "German" }
                        option value="spa_Latn" { "Spanish" }
                    }
                }
            }

            button type="submit" { "Translate" }
        }
    }
}

pub fn translation_result() -> Markup {
    html! {
        translation-result {}
    }
}
```

### Create Pages

```rust
// File: translator-server/src/templates/pages.rs

use maud::{html, Markup};
use super::{layout, components};

pub fn home() -> Markup {
    let content = html! {
        h2 { "Translate Text" }
        (components::translation_form())
        div id="result" { (components::translation_result()) }
    };
    layout::base("Home", content)
}

pub fn stats(translation_count: u64, job_count: u64, redis_connected: bool) -> Markup {
    let content = html! {
        h2 { "Statistics" }
        div class="translation-result" {
            table {
                tr { td { "Cached Translations:" } td { (translation_count) } }
                tr { td { "Active Jobs:" } td { (job_count) } }
                tr {
                    td { "Redis:" }
                    td {
                        @if redis_connected { "Connected" } @else { "Disconnected" }
                    }
                }
            }
        }
    };
    layout::base("Stats", content)
}
```

### Update main.rs

```rust
// File: translator-server/src/main.rs (add module)

mod templates;
```

### Verification: Section 4.1

```bash
cargo build -p translator-server
```

**Expected**: Compiles successfully

---

## Section 4.2: UI Routes (30 min)

### Learning Objective

Create routes that serve HTML pages.

### Create UI Routes

```rust
// File: translator-server/src/routes/ui.rs

use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use redis::AsyncCommands;
use crate::state::SharedState;
use crate::templates::pages;

pub async fn home() -> impl IntoResponse {
    Html(pages::home().into_string())
}

pub async fn stats_page(State(state): State<SharedState>) -> impl IntoResponse {
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

    Html(pages::stats(translation_count, job_count, ping.is_ok()).into_string())
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(home))
        .route("/ui/stats", get(stats_page))
}
```

### Update Routes Module

```rust
// File: translator-server/src/routes/mod.rs

pub mod health;
pub mod jobs;
pub mod stats;
pub mod translate;
pub mod ui;

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
        .merge(ui::router())
        .layer(cors)
        .with_state(state)
}
```

### Verification: Section 4.2

```bash
cargo run -p translator-server &
```

Open http://localhost:3000/ in your browser.

**Expected**: See "Translation Service" heading and form

---

## Section 4.3: Web Component Patterns (30 min)

### Learning Objective

Understand the Web Component architecture.

### TranslationForm Component

The form extends `HTMLFormElement` (customized built-in):

```javascript
class TranslationForm extends HTMLFormElement {
    connectedCallback() {
        this.addEventListener('submit', this);  // handleEvent pattern
    }

    handleEvent(e) {
        if (e.type === 'submit') {
            e.preventDefault();
            this.#handleSubmit();
        }
    }
}
```

Key patterns:

| Pattern | Purpose |
|---------|---------|
| `extends HTMLFormElement` | Preserve form semantics |
| `handleEvent` | No `.bind(this)` needed |
| `#privateFields` | Proper cleanup |
| `FormData` API | No querySelector needed |

### TranslationResult Component

This autonomous element displays status:

```javascript
class TranslationResult extends HTMLElement {
    connectedCallback() {
        document.addEventListener('translation-status', this);
    }

    handleEvent(e) {
        if (e.type === 'translation-status') {
            this.#render(e.detail);
        }
    }
}
```

### Communication Flow

```
TranslationForm                    TranslationResult
     |                                    |
     |-- dispatchEvent('translation-status', {status: 'pending'}) -->
     |                                    |
     |                              [Re-renders]
     |                                    |
     |-- dispatchEvent('translation-status', {status: 'complete'}) -->
     |                                    |
     |                              [Shows results]
```

### Verification: Section 4.3

Open browser DevTools and add:

```javascript
document.addEventListener('translation-status', e => console.log('Status:', e.detail));
```

Submit a translation and watch events flow.

---

## Section 4.4: Testing the UI (30 min)

### Learning Objective

Verify the complete flow works end-to-end.

### Test Flow

1. Open http://localhost:3000/
2. Enter text: "Hello"
3. Select French as target
4. Click Translate
5. Watch:
   - Button disables
   - Spinner appears
   - Results show

**Expected result**:

```
Original: Hello
Translated: [mock::fra_Latn] Hello
```

### Test Edge Cases

| Test | Action | Expected |
|------|--------|----------|
| Empty input | Clear textarea, submit | Error message |
| Same language | Set both to English | Error message |
| Multiple lines | Enter 3 lines | 3 translations |

### Test Stats Page

1. Open http://localhost:3000/ui/stats
2. See current counts
3. Translate something
4. Refresh stats
5. Job count increases

### Verification: Section 4.4

Complete this checklist:

- [ ] Form loads at /
- [ ] Submit shows spinner
- [ ] Results display correctly
- [ ] Stats page shows counts
- [ ] Clear results works

---

## Chapter Summary

You now have a functional web UI:

| Component | Technology |
|-----------|------------|
| Server HTML | Maud templates |
| Client interactivity | Vanilla JavaScript |
| UI components | Web Components (Custom Elements) |
| Styling | CSS custom properties |

### Web Component Patterns Used

| Pattern | Where Used |
|---------|------------|
| Customized built-in | `TranslationForm extends HTMLFormElement` |
| Autonomous element | `TranslationResult extends HTMLElement` |
| handleEvent | Both components |
| Event-based communication | `translation-status` custom event |

### File Structure

```
translator-server/src/
  templates/
    mod.rs
    layout.rs       # Base HTML + CSS + JS
    components.rs   # Reusable Maud components
    pages.rs        # Full page templates
  routes/
    ui.rs           # HTML routes
```

### UI Routes

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/` | Home page with form |
| GET | `/ui/stats` | Statistics page |

---

## Next Chapter

Continue to [Chapter 5: Deploying to Render.com](./05-deployment.md) to:

- Create a multi-stage Dockerfile
- Configure render.yaml
- Deploy to production
