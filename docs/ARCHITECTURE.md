# OurFault – Architecture

This document explains how the PoC is put together and why. It is meant to be
read before changing the code.

## Goals and non-goals

OurFault creates investigations of **activities**. An activity (mission,
experiment, training or other) involves one or more systems. An
investigation records the activity details, administrator-configured
technical sections, the event chronology pasted from the operations log, and
a link to the preliminary checks. It is then published (SharePoint in
production) and distributed by e-mail.

- **Input is operator-controlled.** Chronology rows are selected and copied
  in Excel by the operator. OurFault never opens or reads a workbook, and
  never infers, scores or auto-selects rows. Parsing is deterministic, and the
  operator reviews every row (edit, remove, paste more).
- **The structured investigation is the source of truth.** HTML and PDF are
  outputs rendered from it; the model is never collapsed into markup.
- **External systems stay behind interfaces.** SharePoint, a shared folder,
  Outlook and shared draft storage are adapters. In this PoC they are local
  mocks that never touch the network.
- One desktop process (Tauri 2). No web server, no background services, no
  remote database, no AI, no analytics. All data is fictional.

## Layers

```
React UI (src/)                       Hebrew, RTL, presentation and wizard state only
   │  typed calls in src/api/client.ts
   ▼  Tauri IPC (allow-listed commands)
commands.rs                           input/output translation; admin commands via AppState::admin
   ▼
services/                             use cases: drafts, review/complete, search, distribution, export, administration
   ▼
domain/          render/              pure types and rules │ pure output: DocumentView → HTML / PDF
   ▲
adapters/  (traits in adapters/mod.rs)
   ├─ local_configuration.rs  ConfigurationRepository → configuration.json (revisioned)
   ├─ local_drafts.rs         DraftRepository         → drafts/<id>.json (revisioned)
   ├─ mock_sharepoint.rs      InvestigationPublisher  → local JSON "list" (default)
   ├─ shared_folder.rs        InvestigationPublisher  → <root>/<year>/<number> - <name>.pdf + .json
   ├─ mock_distribution.rs    DistributionAdapter     → local "outbox" JSON
   └─ local_export.rs         ExportSink              → Downloads\OurFault
state.rs                              composition root, work mode, access policy
```

**All business rules live in Rust.** The frontend keeps only UI state
(`features/wizard/wizardState.ts`, a pure reducer; `autosave.ts`, a pure
scheduler) and formatting. One source for each rule:

| Rule | Location |
| --- | --- |
| Activity: required fields, "Other" description, systems, time ordering | `domain/activity.rs` |
| Technical sections: definitions, admin validation, typed values, required fields, station/system references | `domain/sections.rs` |
| Building a complete investigation from a draft | `domain/investigation.rs` |
| Draft content limits, draft id safety | `domain/draft.rs` |
| Status lifecycle (Draft → Completed → Distributed) | `domain/lifecycle.rs` |
| Mail template placeholders, recipient union | `domain/mail.rs` |
| Setup checklist | `domain/configuration.rs` |
| Paste parsing and row validation (unchanged from PR #1) | `domain/log_rows.rs` |
| Number format, parsing, incrementing | `domain/investigation_number.rs` |
| Number allocation with conflict retry; draft conversion | `services/investigations.rs` |
| Document wording (Hebrew) for preview, HTML and PDF | `render/document.rs`, `render/labels.rs` |

Errors cross IPC only as stable codes (`validation` with `{field, code}`,
`paste`, `conflict`, `unavailable`, `notFound`, `forbidden`, `internal`). The
UI maps codes to Hebrew in `src/api/errors.ts`; technical details stay in the
log.

## Investigation model

```
Investigation
├─ number               056-2026 (allocated on completion only)
├─ activity
│  ├─ name, activityType (Mission | Experiment | Training | Other{description})
│  ├─ systems[]         snapshot {id, name}; one or more, no "primary"
│  ├─ status            Active | Completed        (activity status)
│  ├─ planned {start, end}, actual {start?, end?}
│  └─ nightActivity, seniorStaffing
├─ sections[]           snapshot of each configured section + typed cells
├─ rows[]               event chronology (pasted operations-log rows)
├─ preliminaryCheckUrl
└─ sourceDraftId        the draft it was created from (one draft → at most one investigation)
PublishedInvestigation = Investigation + Lifecycle + Publication{destination, url, reference}
```

Rules worth knowing:

- Times are never required while the investigation is a draft: drafts
  save with any time field empty or half-typed, and autosave never
  validates them.
- To complete an investigation, the planned start and end are required.
  The actual start and end are required only when the activity status is
  *Completed*; while it is *Active* both may be empty (or only the start
  filled). Each end must not precede its start; planned and actual windows
  need not match. Missing values (`required`) and invalid values or ranges
  (`invalid_datetime`, `end_before_start`) are listed as separate groups in
  the review summary.
- The investigation date (dashboard column) is the actual start, or the
  planned start while the activity has not actually started.
- The yes/no questions must be answered explicitly (nothing defaults to "no").
- A `System` field in a technical section may only reference one of the
  investigation's own systems. A `Station` field must reference an active
  station.
- Completed investigations store snapshots (system and station names, section
  labels and types), so renaming or deactivating configuration never
  rewrites history.
- Enums carry no display text; labels are a UI or rendering concern.

### Lifecycle

`Draft → Completed → Distributed (→ Distributed again)`. A draft lives in the
draft repository and has no number. Completion allocates the number and
publishes. Distribution appends to the history; `completedAt` and the first
`distributedAt` are kept, and nothing moves backwards. Published
investigations are edited in SharePoint, not in OurFault.

## Technical sections (section builder)

A controlled builder, not a document editor. A section has a name, an order
(its position), an active flag, a mode (*single record* or *repeating rows*)
and ordered fields. A field has a label, an order, *required*, *active* and a
type from a deliberately small set: Text, Number, Boolean, Single Select,
Multi Select, Station, System. Select fields carry their options.

- Admins rename and reorder sections (move up/down), add, rename, reorder,
  retype, deactivate or remove fields, and mark them required.
- Removing a field removes it from the configuration only; completed
  investigations keep their snapshot, and drafts simply ignore values of
  unknown fields.
- The seed contains the two real-world table layouts with placeholder labels
  (`כותרת 1`, `כותרת 2`), in their required order
  (`כותרת 1 | כותרת 2 | מס׳ זנב | מס׳ קרון | מקטע`). No label is referenced by
  code; the real names are set during setup.

## Drafts and autosave

- `DraftRepository` is the boundary; `LocalDraftRepository` (one JSON file per
  draft) is the PoC. A shared implementation (network folder or SharePoint)
  must keep the same contract.
- Every save names the revision it edited. A mismatch is a `conflict`: the
  write is refused and the UI stops autosaving instead of overwriting another
  workstation's changes. The operator's in-memory work is kept.
- Draft ids come from the webview and become file names, so they are
  restricted to `[a-z0-9-]` and validated in the domain and the adapter.
- The UI autosaves 800 ms after the last change, with at most one write in
  flight (`features/wizard/autosave.ts`). An untouched new investigation is
  never saved. The indicator shows *נשמר* / *שומר...* / *שגיאה בשמירה* (with
  retry); leaving with unsaved changes asks first.
- Completion (`services/investigations.rs::complete`): load the draft →
  ask the publisher whether an investigation with this `sourceDraftId`
  already exists (if so, return it) → check the draft is open and at the
  expected revision → validate → allocate and publish → **only then** mark
  the draft converted. If publishing fails, the draft is untouched and no
  number is used.

### One draft, at most one investigation

Completion is idempotent and does not depend on the draft's "converted"
mark (that write may fail after publication succeeded):

1. Every investigation records its `sourceDraftId`.
2. Before publishing, the service looks the draft up at the publisher
   (`find_by_source_draft`) and returns the existing investigation if there
   is one (`alreadyExisted: true`; the UI says that nothing new was
   created). A retry after a partial failure therefore returns the same
   investigation without allocating a number or writing a record, and
   repairs the draft's converted mark.
3. The publisher enforces the invariant itself, so two simultaneous
   attempts cannot both publish: `publish` refuses a second investigation
   from the same draft with `DraftAlreadyPublished`, and the service then
   returns the one that exists.
   - Mock SharePoint: checked under the same lock as the number (a real
     list would use a unique-values column for the source draft id).
   - Shared folder: attempts for one draft are serialised by a
     create-if-absent lock file (`.drafts/<id>.lock`, taken over when older
     than two minutes after a crash); inside the lock an existing
     investigation for the draft is detected before anything is written.
     `.drafts/<id>.marker` indexes draft → number; without it, the records
     themselves are scanned.

## Investigation numbers

`{running number}-{year}`, at least three digits (`001-2026`, `056-2026`,
`1000-2026`). The running number restarts each calendar year. The review step
shows the *expected* number; the publisher allocates it:

1. The service proposes the next number from the publisher's highest number.
2. `InvestigationPublisher::publish` must be an atomic create-if-absent and
   fail with `NumberTaken` (SharePoint: a unique column or ETag-guarded
   counter; shared folder: `create_new` on the record file).
3. On `NumberTaken` the service retries with a fresh number (bounded). The
   success screen explains if the number changed.

## Rendering and publishing

```
Investigation / Draft ──▶ DocumentView ──▶ HtmlRenderer ──▶ SharePoint rich-text body (mock)
                                      └──▶ PdfRenderer  ──▶ export, shared-folder publisher
```

- `DocumentView` is a neutral list of labelled field groups and tables. The
  in-app preview renders the same view, so preview, PDF and HTML never
  disagree. Drafts get a `draftNotice` ("טיוטה — לא להפצה").
- `HtmlRenderer` emits semantic, escaped, `dir="rtl"` HTML (`article`,
  `section`, `dl`, `table/th[scope]`, `bdi` for left-to-right values). It is
  never shown to operators.
- The mock SharePoint list item carries **both** mappings a real adapter may
  need: dashboard columns (`number`, `systems`, `activityName`, `date`,
  `activityType`, `status`), the full structured investigation, and
  `bodyHtml` for a single rich-text field.
- An unreachable destination returns `unavailable`: nothing is written, no
  number is allocated, the draft stays as it is. No offline synchronisation.

### PDF

PDF generation is a small in-house writer (`render/pdf/`): A4, right-to-left,
field grids and tables with repeated headers and page breaks inside long
cells, page numbers, and for drafts a banner plus a diagonal watermark on
every page. Text is searchable/copyable (ToUnicode map).

Options considered: a headless browser or WebView2 `PrintToPdf` (huge or
Windows-GUI-only, not usable by a headless folder publisher, hard to test in
CI); typesetting engines such as typst (very large); `printpdf`/`genpdf`
(no right-to-left support, still need our own layout). The chosen approach
adds two small, dependency-free crates: `ttf-parser` (glyph ids and metrics)
and `unicode-bidi` (the Unicode Bidirectional Algorithm, to reorder mixed
Hebrew/Latin/number lines). `miniz_oxide` (already in the dependency tree)
compresses streams. The bundled font is Alef (SIL Open Font License,
`src-tauri/assets/fonts/OFL.txt`), embedded in each PDF (~90 KB).
Limitations: no complex shaping (not needed for Hebrew without niqqud), no
kerning, no hyphenation.

A draft can be exported only when the call carries `confirmIncomplete: true`,
which the UI sends after the operator confirms *"התחקיר עדיין לא הושלם או
הופץ. הקובץ שייווצר עלול להיות חלקי. האם להמשיך?"*. Files are written to
`Downloads\OurFault` (never overwriting); the webview never supplies a path.
A native "Save as" dialog can be added later by calling
`tauri-plugin-dialog` from Rust only, without granting the webview any
dialog permission.

## Distribution

One administrator-editable template (subject, body, link text) with
`{{ACTIVITY_NAME}}`, `{{SYSTEMS}}`, `{{INVESTIGATION_NUMBER}}`,
`{{INVESTIGATION_LINK}}`. Unknown placeholders are rejected when saving.
Values are inserted as text (HTML-escaped in the HTML body) and never
re-interpreted as placeholders. `{{INVESTIGATION_LINK}}` renders as a
hyperlink whose text is the link text ("תחקיר"); the plain-text body writes
the URL out. Recipients are the union of the involved systems' lists,
de-duplicated (addresses are lower-cased on save).

No sender or signature is added: the real `DistributionAdapter` is expected
to send as the signed-in Outlook user, whose own signature applies. Any
Outlook-specific automation belongs inside that adapter. Distribution
records the new status; if the message was sent but recording failed, the UI
says so instead of claiming full success.

## Work modes (regular / admin)

The entry screen offers **כניסה רגילה** and **מצב מנהל**. These are work
modes, not accounts: there is no username or password, and choosing admin
mode does not identify anyone. Structurally:

- The mode is held in Rust (`AppState`), not only in React.
- Administration services (`services/configuration.rs::Administration`)
  require an `AdminGrant`, which only `AppState::admin()` can create, and only
  in admin mode. Every admin command goes through it; regular mode gets
  `forbidden` (tested in `state.rs`).
- An `AccessPolicy` decides which modes may be entered. The PoC policy allows
  admin mode unless `OURFAULT_ADMIN_MODE=disabled`. A real deployment
  replaces it with environment permissions (e.g. a directory group of the
  Windows user) without changing commands or UI.
- Regular mode shows no admin screens; admin mode shows only configuration.
- The Windows user name is recorded as the author of drafts and status
  changes. It is informational, not an authentication mechanism.

Initial setup and administration are the same screens over the same
configuration. The setup checklist is computed from the current
configuration (`Configuration::setup_status`), so it stays available and
reflects later changes. Required items (an active system, a valid mail
template, a publication target) gate completion, not draft saving.

## Local data and migration

Local JSON files in the per-user data directory
(`%APPDATA%\com.ourfault.desktop`; override with `OURFAULT_DATA_DIR`):

| File | Owner |
| --- | --- |
| `configuration.json` | `LocalConfigurationRepository` |
| `drafts/<id>.json` | `LocalDraftRepository` |
| `mock-sharepoint/investigations-v2.json` | `MockSharePoint` (stands in for the list) |
| `mock-mail/outbox-v2.json` | `MockDistribution` |

Every file carries `"schemaVersion": 2`. Writes are atomic (temp file +
rename). A corrupt file, or one with another schema version, is reported and
never overwritten; the UI then shows a startup error.

**Migration from PR #1:** intentionally none. PR #1 data was fictional seed
data in a different model (one system per investigation, per-system
templates). The new layout uses new file names, so on first start the PR #1
files (`systems.json`, `mock-sharepoint/investigations.json`,
`mock-mail/outbox.json`) are left untouched and ignored, and fresh fictional
data is seeded. Delete them, or the whole data directory, at will.

## Security boundaries

| Concern | Measure |
| --- | --- |
| Webview privileges | One capability (`capabilities/main-window.json`) granting only the 29 OurFault commands (generated in `build.rs`). No core, fs, dialog, clipboard, shell, http or opener permissions. `withGlobalTauri: false`. |
| Work modes | Admin operations require an `AdminGrant` created only by `AppState::admin()` in admin mode; the access policy can withhold admin mode entirely. |
| Input | Pasted text is bounded and cleaned (as in PR #1). Draft content has size/row limits on every save. All content is re-validated in Rust on review and completion. Admin configuration is validated in the domain. |
| File system | The webview never supplies paths. Draft ids are restricted to a safe alphabet; export and publication file names are sanitised (no separators, reserved characters or device names); exports never overwrite. |
| Content injection | Values are rendered as React text only; no `dangerouslySetInnerHTML`, no `eval`. Generated HTML (SharePoint body, e-mail) escapes every value. |
| Network | No outbound requests anywhere. CSP `connect-src ipc: http://ipc.localhost`. The navigation guard keeps the webview on the app origin. The preliminary-checks link is validated structurally and copied, never opened or fetched. |
| Concurrency | Revision checks on configuration and drafts; atomic create-if-absent for investigation numbers; at most one investigation per draft (`sourceDraftId`, enforced by the publisher). |
| Errors | Only codes reach the UI; paths and library errors go to the log. |
| Supply chain | Small dependency set, pinned by `Cargo.lock` / `package-lock.json`. CI uses only GitHub-owned actions. |

## Replacing the mocks

1. Implement the trait in a new file under `src-tauri/src/adapters/`:
   `InvestigationPublisher` (SharePoint via Microsoft Graph: create the list
   item, map the unique-number conflict to `NumberTaken` and connectivity
   failures to `Unavailable`, return the item URL), `DraftRepository` and
   `ConfigurationRepository` (shared storage, honouring revisions),
   `DistributionAdapter` (Outlook, as the signed-in user).
2. Construct it in `Backend::open` (`state.rs`).
3. Keep secrets in Rust (OS credential store or managed configuration),
   never in the frontend or source. Perform network access in Rust; the
   webview CSP stays closed.
4. To open an item in SharePoint, add a narrow Rust command that opens a URL
   **only if it belongs to the configured SharePoint site**. Do not add a
   general "open URL" capability.

Nothing in `domain/`, `services/`, `commands.rs` or the UI needs to change.

## Decisions and known limitations

- **One template and one publication target for all systems.** With
  multi-system activities, per-system templates or lists would conflict.
  Systems keep their distribution lists and an (unused, preserved)
  `metadata` map for future integration values.
- **Systems and stations cannot be deleted**, only deactivated.
- **Frontend types** in `src/api/types.ts` mirror the Rust serde types by
  hand; generate them (e.g. `ts-rs`) if the API keeps growing.
- **Draft storage is local** in the PoC; drafts are therefore per
  workstation until a shared `DraftRepository` exists.
- **PDF export location** is fixed (see above).
- Internal logging is `stderr` only; add a rolling log file before production.
- Light theme only; the visual redesign and dark mode are a separate PR.

## Dependencies

| Dependency | Why |
| --- | --- |
| `tauri`, `tauri-build` | Desktop shell and IPC |
| `chrono` | Local date/time |
| `url` | URL validation, file URLs (already transitive via Tauri) |
| `serde`, `serde_json`, `thiserror` | Serialisation and error types |
| `ttf-parser`, `unicode-bidi`, `miniz_oxide` | PDF export (see above) |
| `react`, `react-dom` | UI |
| `@tauri-apps/api` | Typed `invoke` |
| dev: `vite`, `@vitejs/plugin-react`, `typescript`, `vitest`, `@tauri-apps/cli` | Build and tests |

No UI kit, router, state library, icon package, spreadsheet library or
clipboard plugin.
