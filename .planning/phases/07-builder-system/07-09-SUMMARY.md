---
phase: 07-builder-system
plan: 09
subsystem: web
tags: [react, tanstack-router, zustand, builder, wizard, ui]

# Dependency graph
requires:
  - phase: 07-08
    provides: "REST API endpoints for builder session lifecycle"
provides:
  - "Builder landing page at /builder with description input and Forge links"
  - "Step-by-step wizard at /builder/wizard with phase indicator, step content, and preview panel"
  - "Builder API client (TypeScript) mirroring Rust builder types"
  - "Zustand store for wizard conversation flow with turn history"
  - "Wizard step, preview, and review components"
affects: [08-workflows-pipelines]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "useRef guard pattern to prevent useEffect re-triggering on API error"
    - "Zustand store with turn history for wizard back navigation"
    - "TanStack Router file-based routing for builder sub-pages"

key-files:
  created:
    - apps/web/src/routes/builder/index.tsx
    - apps/web/src/routes/builder/wizard.tsx
    - apps/web/src/lib/api/builder.ts
    - apps/web/src/stores/builder-store.ts
    - apps/web/src/components/builder/wizard-step.tsx
    - apps/web/src/components/builder/builder-preview.tsx
    - apps/web/src/components/builder/builder-review.tsx
  modified: []

key-decisions:
  - "Landing page handles both wizard and Forge entry points with description passthrough via query params"
  - "Wizard delegates step rendering to WizardStep -- does not control step flow, just renders whatever turn the API returns"
  - "useRef guard prevents infinite API retry loop when startSession fails (isLoading flip + null sessionId would re-trigger)"

patterns-established:
  - "Builder API client with typed DTOs matching Rust types (BuilderTurn, BuilderConfig, BuilderAnswer, etc.)"
  - "Phase-based step indicator driven by server-returned phase field"

# Metrics
duration: 12min
completed: 2026-02-14
---

# Phase 7 Plan 9: Web UI Builder Wizard Summary

**Builder landing page and step-by-step wizard with API client, state store, and preview panel**

## Performance

- **Duration:** 12 min
- **Started:** 2026-02-14T14:00:00Z
- **Completed:** 2026-02-14T14:32:00Z
- **Tasks:** 2 (+ 1 post-checkpoint bug fix)
- **Files modified:** 7 created

## Accomplishments
- Builder landing page at `/builder` with description input, "Start Wizard" button, and links to Forge chat
- Step-by-step wizard at `/builder/wizard` with phase indicator bar, step content, and live preview panel
- TypeScript API client mirroring all Rust builder types (BuilderTurn, BuilderConfig, BuilderAnswer, BuilderPhase)
- Zustand store managing wizard conversation flow with full turn history for back navigation
- WizardStep component rendering multi-choice options with descriptions
- BuilderPreview component showing live configuration preview
- BuilderReview component showing full summary with raw file toggle

## Task Commits

Each task was committed atomically:

1. **Task 1: Builder API client and state store** - `7d78e43` (feat)
2. **Task 2: Wizard UI pages and components** - `06ed383` (feat)
3. **Bug fix: Infinite retry loop prevention** - `761fc60` (fix)

## Files Created/Modified
- `apps/web/src/routes/builder/index.tsx` - Landing page with description input, wizard start, Forge links, resume drafts section
- `apps/web/src/routes/builder/wizard.tsx` - Step-by-step wizard with phase indicator, step content, back navigation, preview panel
- `apps/web/src/lib/api/builder.ts` - TypeScript API client with types and functions (createBuilderSession, submitAnswer, assembleBot, getSession, listDrafts, deleteSession)
- `apps/web/src/stores/builder-store.ts` - Zustand store for wizard state with turn history for back navigation
- `apps/web/src/components/builder/wizard-step.tsx` - Step content renderer with multi-choice options
- `apps/web/src/components/builder/builder-preview.tsx` - Live preview panel component
- `apps/web/src/components/builder/builder-review.tsx` - Review step with full config summary and raw file toggle

## Decisions Made
- **useRef guard pattern:** Added ref guard to prevent useEffect from re-triggering startSession on API errors (isLoading→false + null sessionId would cause infinite loop)
- **Server-driven wizard:** Wizard renders whatever BuilderTurn the API returns rather than controlling steps locally
- **Query param passthrough:** Description from landing page passed to wizard via search params

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Infinite API retry loop in wizard useEffect**
- **Found during:** Manual verification
- **Issue:** useEffect condition `!sessionId && description && !isLoading` re-triggers after every failed startSession call
- **Fix:** Added `useRef(false)` guard to ensure startSession only fires once per mount
- **Files modified:** apps/web/src/routes/builder/wizard.tsx
- **Committed in:** 761fc60

---

**Total deviations:** 1 auto-fixed (runtime bug)
**Impact on plan:** Minor defensive fix, no scope change.

## Verification Notes
- TypeScript compiles cleanly (`tsc --noEmit` exit 0)
- Routes render correctly in browser (verified via Playwright)
- 500 errors from API are expected when no LLM provider is configured (infrastructure concern, not code bug)
- Infinite retry loop confirmed fixed (single API call per page load)

## Self-Check: PASSED

---
*Phase: 07-builder-system*
*Completed: 2026-02-14*
