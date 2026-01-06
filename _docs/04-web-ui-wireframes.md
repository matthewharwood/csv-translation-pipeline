# Web UI Wireframes - MVP Component Architecture

**Designer**: Arya (Staff Design Systems Engineer)
**Version**: 1.0 MVP
**Last Updated**: 2026-01-05

---

## Design Tokens Reference

Before building, reference these tokens throughout implementation:

```
SPACING:     xs=4px  sm=8px  md=16px  lg=24px  xl=32px
RADII:       sm=4px  lg=8px
COLORS:      primary=#0066cc  success=#006600  warning=#996600  error=#660000
TYPOGRAPHY:  h1=1.5rem  h2=1.25rem  body=1rem  small=0.875rem
TOUCH:       min=44px (prefer 48px for primary actions)
BREAKPOINT:  mobile=320px  tablet=768px  desktop=1024px
```

---

## 1. Page Layout Shell

```
┌─────────────────────────────────────────────────────────────────┐
│ ╔═══════════════════════════════════════════════════════════╗   │
│ ║                    <PageShell>                            ║   │
│ ║  max-width: 800px | margin: 0 auto | padding: --md        ║   │
│ ╠═══════════════════════════════════════════════════════════╣   │
│ ║                                                           ║   │
│ ║  ┌─────────────────────────────────────────────────────┐  ║   │
│ ║  │              <Header>                               │  ║   │
│ ║  │  border-bottom: 2px solid | padding-bottom: --md    │  ║   │
│ ║  └─────────────────────────────────────────────────────┘  ║   │
│ ║                         ↓ margin: --xl                    ║   │
│ ║  ┌─────────────────────────────────────────────────────┐  ║   │
│ ║  │              <MainContent>                          │  ║   │
│ ║  │  min-height: 60vh                                   │  ║   │
│ ║  │                                                     │  ║   │
│ ║  │  [Page-specific content renders here]               │  ║   │
│ ║  │                                                     │  ║   │
│ ║  └─────────────────────────────────────────────────────┘  ║   │
│ ║                         ↓ margin: --xl                    ║   │
│ ║  ┌─────────────────────────────────────────────────────┐  ║   │
│ ║  │              <Footer>                               │  ║   │
│ ║  │  border-top: 1px solid | padding-top: --md          │  ║   │
│ ║  └─────────────────────────────────────────────────────┘  ║   │
│ ║                                                           ║   │
│ ╚═══════════════════════════════════════════════════════════╝   │
└─────────────────────────────────────────────────────────────────┘
```

**Component Boundary:** `layout.rs::base()`

---

## 2. Header Component

```
┌─────────────────────────────────────────────────────────────────┐
│  <Header>                                                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <Heading>                                                 │  │
│  │                                                           │  │
│  │  Translation Service                                      │  │
│  │  ────────────────────                                     │  │
│  │  font-size: 1.5rem | font-weight: 700 | margin: 0         │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                          ↓ margin: --sm                         │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <Nav>                                                     │  │
│  │                                                           │  │
│  │  [Home]  |  [Stats]                                       │  │
│  │    ↑          ↑                                           │  │
│  │  <NavLink>  <NavLink>                                     │  │
│  │  color: --primary | text-decoration: underline on hover   │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Component Boundary:** Inline in `layout.rs::base()`

**Accessibility:**
- `<nav>` landmark for screen readers
- Links have visible focus states (3:1 contrast ratio)
- Current page indicated with `aria-current="page"`

---

## 3. Translation Form

```
┌─────────────────────────────────────────────────────────────────┐
│  <TranslationForm is="translation-form">                        │
│  ──────────────────────────────────────                         │
│  background: --surface | padding: --lg | radius: --lg           │
│  box-shadow: 0 2px 4px rgba(0,0,0,0.1)                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <FormGroup: TextInput>                                    │  │
│  │                                                           │  │
│  │  Text to translate (one per line):  [Load Example]        │  │
│  │  ─────────────────────────────────   ↑                    │  │
│  │  <Label>                            <ExampleLoader>       │  │
│  │                                     is="example-loader"   │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │                                                     │  │  │
│  │  │  Hello                                              │  │  │
│  │  │  Goodbye                                            │  │  │
│  │  │  How are you?                                       │  │  │
│  │  │                                                     │  │  │
│  │  │  <Textarea id="texts">                              │  │  │
│  │  │  min-height: 120px | resize: vertical               │  │  │
│  │  │  placeholder visible when empty                     │  │  │
│  │  │                                                     │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                          ↓ margin: --md                         │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <FormGroup: ProviderSelect>                               │  │
│  │                                                           │  │
│  │  ┌─────────────────────────┐  ┌─────────────────────────┐ │  │
│  │  │ Translation Provider:   │  │ <ProviderHint>          │ │  │
│  │  │ ┌─────────────────────┐ │  │ Instant fake            │ │  │
│  │  │ │ Mock (Testing)    ▼ │ │  │ translations for        │ │  │
│  │  │ └─────────────────────┘ │  │ testing                 │ │  │
│  │  │ <Select id="provider">  │  │                         │ │  │
│  │  └─────────────────────────┘  └─────────────────────────┘ │  │
│  │                                                           │  │
│  │  Options:                                                 │  │
│  │  - Mock (Testing)          → "Instant fake translations"  │  │
│  │  - NLLB REST (200+ langs)  → "Requires NLLB server"       │  │
│  │  - MarianMT (Pure Rust)    → "Requires libtorch"          │  │
│  │  - Gemini (Cloud LLM)      → "Requires GEMINI_API_KEY"    │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                          ↓ margin: --md                         │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <FormRow: LanguageSelectors>                              │  │
│  │ display: grid | grid-template-columns: 1fr 1fr | gap: --md│  │
│  │                                                           │  │
│  │  ┌────────────────────┐    ┌────────────────────┐         │  │
│  │  │ Source language:   │    │ Target language:   │         │  │
│  │  │ ┌────────────────┐ │    │ ┌────────────────┐ │         │  │
│  │  │ │ English      ▼ │ │    │ │ French       ▼ │ │         │  │
│  │  │ └────────────────┘ │    │ └────────────────┘ │         │  │
│  │  │ <Select>           │    │ <Select>           │         │  │
│  │  │ id="src_lang"      │    │ id="tgt_lang"      │         │  │
│  │  └────────────────────┘    └────────────────────┘         │  │
│  │                                                           │  │
│  │  Language Options (both selects):                         │  │
│  │  - English (eng_Latn)                                     │  │
│  │  - French (fra_Latn)                                      │  │
│  │  - German (deu_Latn)                                      │  │
│  │  - Spanish (spa_Latn)                                     │  │
│  │  - Chinese Simplified (zho_Hans)                          │  │
│  │  - Japanese (jpn_Jpan)                                    │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                          ↓ margin: --md                         │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <noscript>                                                │  │
│  │ Only visible when JS disabled                             │  │
│  │                                                           │  │
│  │  ⚠ Note: JavaScript is disabled. Translation slower.     │  │
│  │  color: --warning | background: --warning-bg              │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                          ↓ margin: --md                         │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <SubmitButton>                                            │  │
│  │                                                           │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │                                                     │  │  │
│  │  │                    Translate                        │  │  │
│  │  │                                                     │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  min-height: 48px | min-width: 120px                      │  │
│  │  background: --primary | color: white | radius: --sm      │  │
│  │  :disabled → background: #ccc, cursor: not-allowed        │  │
│  │  :hover:not(:disabled) → background: --primary-hover      │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Component Boundary:** `components.rs::translation_form()`

**Web Component:** `TranslationForm extends HTMLFormElement`
- Uses `is="translation-form"` customized built-in
- Handles submit via `handleEvent` pattern
- Dispatches `translation-status` custom events

**Accessibility:**
- All inputs have visible labels (never placeholder-only)
- Form validates on submit, not on keystroke
- Error messages adjacent to related fields
- Submit button 48px touch target

---

## 4. Translation Result States

### 4a. Idle State (Initial)

```
┌─────────────────────────────────────────────────────────────────┐
│  <TranslationResult status="idle">                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  [Empty - no content rendered]                                  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 4b. Pending State (Loading)

```
┌─────────────────────────────────────────────────────────────────┐
│  <TranslationResult status="pending">                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <StatusPending>                                           │  │
│  │ background: --warning-bg | color: --warning | radius: --sm│  │
│  │ padding: --sm --md | display: flex | align-items: center  │  │
│  │                                                           │  │
│  │  ◠ ◡   Translating with Gemini... (Job: a1b2c3d4...)     │  │
│  │  ↑                                                        │  │
│  │  <Spinner>                                                │  │
│  │  16x16px | border: 2px | animation: spin 1s linear        │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 4c. Complete State (Success)

```
┌─────────────────────────────────────────────────────────────────┐
│  <TranslationResult status="complete">                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <StatusComplete>                                          │  │
│  │ background: --success-bg | color: --success | radius: --sm│  │
│  │ display: flex | justify-content: space-between            │  │
│  │                                                           │  │
│  │  ✓ Translation complete!                    [Clear]       │  │
│  │                                               ↑           │  │
│  │                                            <ClearButton>  │  │
│  │                                            btn-secondary  │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                          ↓ margin: --md                         │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <ResultCard>                                              │  │
│  │ background: --surface | padding: --md | radius: --lg      │  │
│  │ box-shadow: --shadow                                      │  │
│  │                                                           │  │
│  │  Translated to: French                                    │  │
│  │  ═══════════════════════                                  │  │
│  │  <CardHeading> border-bottom: 1px solid #eee              │  │
│  │                                                           │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │ <TranslationItem>                                   │  │  │
│  │  │ display: grid | grid-template-columns: 1fr 1fr      │  │  │
│  │  │ gap: --md | padding: --sm 0 | border-bottom: 1px    │  │  │
│  │  │                                                     │  │  │
│  │  │  Original:              Translated:                 │  │  │
│  │  │  Hello, how are you?    Bonjour, comment allez-vous?│  │  │
│  │  │                                                     │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │ <TranslationItem>                                   │  │  │
│  │  │                                                     │  │  │
│  │  │  Original:              Translated:                 │  │  │
│  │  │  I am learning Rust.    J'apprends Rust.            │  │  │
│  │  │                                                     │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │ <TranslationItem> (last - no border-bottom)         │  │  │
│  │  │                                                     │  │  │
│  │  │  Original:              Translated:                 │  │  │
│  │  │  This is a test.        C'est un test.              │  │  │
│  │  │                                                     │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 4d. Error State

```
┌─────────────────────────────────────────────────────────────────┐
│  <TranslationResult status="error">                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <StatusError>                                             │  │
│  │ background: --error-bg | color: --error | radius: --sm    │  │
│  │ padding: --sm --md                                        │  │
│  │                                                           │  │
│  │  ✗ Error: Source and target language must be different   │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Component Boundary:** `TranslationResult extends HTMLElement`
- Autonomous custom element (not built-in extension)
- Attribute-driven: `status="idle|pending|complete|error"`
- Listens for `translation-status` events
- Re-renders on `attributeChangedCallback`

---

## 5. Stats Page

```
┌─────────────────────────────────────────────────────────────────┐
│  <StatsPage>                                                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <PageHeading>                                             │  │
│  │                                                           │  │
│  │  Service Statistics                                       │  │
│  │  ══════════════════                                       │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                          ↓ margin: --md                         │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <StatsCard>                                               │  │
│  │ background: --surface | padding: --md | radius: --lg      │  │
│  │                                                           │  │
│  │  Cache Stats                                              │  │
│  │  ───────────                                              │  │
│  │                                                           │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │ <StatsTable>                                        │  │  │
│  │  │                                                     │  │  │
│  │  │  Cached Translations:     42                        │  │  │
│  │  │  Active Jobs:              3                        │  │  │
│  │  │  Redis Connected:         ✓ Yes (green)             │  │  │
│  │  │                     or    ✗ No  (red)               │  │  │
│  │  │                                                     │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                          ↓ margin: --md                         │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ <DangerAction>                                            │  │
│  │                                                           │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │                                                     │  │  │
│  │  │            Clear Translation Cache                  │  │  │
│  │  │                                                     │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  background: --error | color: white                       │  │
│  │  Form: POST /stats/cache with _method=DELETE              │  │
│  │                                                           │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Component Boundary:** `pages.rs::stats_page()`

---

## 6. Mobile Responsive Breakpoints

```
DESKTOP (≥768px)                    MOBILE (<768px)
─────────────────                   ─────────────────

┌─────────────────────┐             ┌─────────────────┐
│ Source:  │ Target:  │             │ Source:         │
│ [Select] │ [Select] │             │ [Select      ▼] │
└─────────────────────┘             ├─────────────────┤
                                    │ Target:         │
FormRow: grid 1fr 1fr               │ [Select      ▼] │
                                    └─────────────────┘

                                    FormRow: grid 1fr (stacked)


┌────────────┬────────────┐         ┌─────────────────┐
│ Original:  │ Translated:│         │ Original:       │
│ Hello      │ Bonjour    │         │ Hello           │
└────────────┴────────────┘         ├─────────────────┤
                                    │ Translated:     │
TranslationItem: grid 1fr 1fr       │ Bonjour         │
                                    └─────────────────┘

                                    TranslationItem: grid 1fr
```

**CSS Media Query:**
```css
@media (max-width: 767px) {
  .form-row,
  .translation-item {
    grid-template-columns: 1fr;
  }
}
```

---

## 7. Component Hierarchy

```
<PageShell>                          layout.rs::base()
├── <Header>                         inline
│   ├── <Heading>                    h1
│   └── <Nav>                        nav > a
├── <MainContent>                    main
│   └── [Page Content]
│       │
│       ├── HOME PAGE ─────────────  pages.rs::home()
│       │   ├── <PageHeading>        h2
│       │   ├── <PageIntro>          p
│       │   ├── <TranslationForm>    components.rs::translation_form()
│       │   │   ├── <FormGroup>      Text input
│       │   │   │   ├── <Label>
│       │   │   │   ├── <ExampleLoader>
│       │   │   │   └── <Textarea>
│       │   │   ├── <FormRow>        Provider + hint
│       │   │   │   ├── <Select>     Provider
│       │   │   │   └── <ProviderHint>
│       │   │   ├── <FormRow>        Languages
│       │   │   │   ├── <Select>     src_lang
│       │   │   │   └── <Select>     tgt_lang
│       │   │   ├── <noscript>
│       │   │   └── <SubmitButton>
│       │   └── <TranslationResult>  components.rs::translation_result_placeholder()
│       │       └── [State-dependent content]
│       │
│       └── STATS PAGE ────────────  pages.rs::stats_page()
│           ├── <PageHeading>        h2
│           ├── <StatsCard>
│           │   └── <StatsTable>
│           └── <DangerAction>       Clear cache button
│
└── <Footer>                         inline
    └── <FooterText>                 p
```

---

## 8. Web Component Event Flow

```
User Action                    Component                     Event
───────────                    ─────────                     ─────

[Click Translate] ─────────→ TranslationForm
                              │
                              ├─→ preventDefault()
                              ├─→ Validate form
                              ├─→ POST /translate
                              │
                              └─→ dispatch('translation-status', {
                                    status: 'pending',
                                    message: 'Submitting...'
                                  })
                                        │
                                        ▼
                              TranslationResult.handleEvent()
                              │
                              ├─→ setAttribute('status', 'pending')
                              └─→ #render() → Show spinner


[Poll /jobs/:id] ────────────→ TranslationForm
                              │
                              └─→ dispatch('translation-status', {
                                    status: 'complete',
                                    translations: {...}
                                  })
                                        │
                                        ▼
                              TranslationResult.handleEvent()
                              │
                              ├─→ setAttribute('status', 'complete')
                              └─→ #render() → Show results


[Click Clear] ───────────────→ TranslationResult
                              │
                              └─→ setAttribute('status', 'idle')
                                  │
                                  └─→ attributeChangedCallback()
                                      │
                                      └─→ #render() → Clear content
```

---

## 9. Accessibility Checklist

| Requirement | Component | Implementation |
|-------------|-----------|----------------|
| Visible focus | All interactive | `:focus-visible` with 3:1 contrast |
| Touch targets | Buttons, selects | min 44px, primary 48px |
| Labels | Form inputs | Visible `<label>`, never placeholder-only |
| Error messages | Form validation | Adjacent, color + icon |
| Loading state | TranslationResult | Spinner + text, `aria-busy="true"` on form |
| Landmark | Header, Main, Footer | `<header>`, `<main>`, `<footer>` |
| Keyboard | Full form | Tab order, Enter submits |
| Screen reader | Status updates | `aria-live="polite"` on result area |
| Reduced motion | Spinner | `@media (prefers-reduced-motion)` |
| Color alone | Status indicators | Icon + text, not just color |

---

## 10. Implementation Mapping

| Wireframe Component | Rust Module | HTML Element |
|---------------------|-------------|--------------|
| PageShell | `layout.rs::base()` | `<body>` wrapper |
| Header | inline in `base()` | `<header>` |
| TranslationForm | `components.rs::translation_form()` | `<form is="translation-form">` |
| ExampleLoader | inline button | `<button is="example-loader">` |
| TranslationResult | `components.rs::translation_result_placeholder()` | `<translation-result>` |
| StatsPage | `pages.rs::stats_page()` | Full page render |
| Home | `pages.rs::home()` | Full page render |

---

## UX Evaluation Summary

**Overall Score: 4.2/5.0 (Good)**

### Strengths
- ✓ Clear visual hierarchy with consistent spacing
- ✓ Accessible form with visible labels
- ✓ Progressive enhancement (works without JS)
- ✓ Clear loading and error states
- ✓ Semantic HTML structure

### Recommendations

| Priority | Issue | Fix |
|----------|-------|-----|
| P1 | Add `aria-live="polite"` to result container | Screen reader announces changes |
| P1 | Increase touch target on Clear button to 44px | Currently btn-secondary may be too small |
| P2 | Add keyboard shortcut hint (Cmd+Enter to submit) | Power user efficiency |
| P2 | Consider optimistic UI for faster perceived performance | Show result skeleton immediately |

---

*Wireframes by Arya - Staff Design Systems Engineer*
