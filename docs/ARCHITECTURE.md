# OurFault – Architecture

This document explains how the PoC is put together and why. It is meant to be
read before changing the code.

## Goals and non-goals

OurFault turns rows of an operations-log workbook into an investigation
report: select rows → choose a system → paste the preliminary-checks link →
preview → create → distribute.

The PoC deliberately stays small:

- One desktop process (Tauri 2). No web server, no background services.
- Real, local Excel reading. SharePoint and e-mail are **mocks** that write
  local JSON files and never touch the network.
- All data is fictional.

## Layers

```
React UI (src/)                       Hebrew, RTL, presentation only
   │  typed calls in src/api/client.ts
   ▼  Tauri IPC (allow-listed commands)
commands.rs                           input/output translation, auth checks
   ▼
services/                             use cases: numbering, create, distribute, admin
   ▼
domain/                               pure types and rules, no I/O
   ▲
adapters/  (implement traits in adapters/mod.rs)
   ├─ excel/                 real .xlsx reading (calamine + small fill reader)
   ├─ mock_sharepoint.rs     SharePointAdapter   → local JSON
   ├─ mock_distribution.rs   DistributionAdapter → local "outbox" JSON
   └─ json_system_repository.rs  SystemRepository → local JSON
state.rs                              composition root: picks the adapter implementations
```

**All business rules live in Rust.** That is where the trust boundary is, and
where file-system and (later) network access must happen anyway. The frontend
keeps only UI state (`features/wizard/wizardState.ts`, a pure reducer) and
formatting. There is exactly one source for each rule:

| Rule | Location |
| --- | --- |
| Number format, parsing, incrementing | `domain/investigation_number.rs` |
| Link, e-mail and text validation | `domain/validation.rs` |
| Building an investigation (active system, rows, link, template snapshot) | `domain/investigation.rs` |
| System configuration validation | `domain/system.rs` |
| Number allocation with conflict retry | `services/investigations.rs` |
| Distribution message content | `domain/distribution.rs` |

Errors cross IPC only as stable codes (`{kind: "validation", errors: [{field, code}]}`).
The UI maps codes to Hebrew text in `src/api/errors.ts`. Technical details stay
in the log (`log_internal`).

## Project structure

```
src/                     React + TypeScript (strict)
  api/                   IPC types, the typed client, error → Hebrew messages
  features/home          home screen: new investigation, search, recent list
  features/wizard        the three-step creation wizard + success step
  features/investigation shared document view, saved-investigation screen, distribution dialog
  features/admin         system administration
  ui/                    small shared controls (buttons, fields, dialog, icons)
  styles/                design tokens and CSS (logical properties → native RTL)
src-tauri/
  src/                   Rust application (see layers above)
  seed/                  fictional systems and past investigations (first-run seed)
  capabilities/          the single Tauri capability (explicit command allow-list)
demo/                    fictional operations-log workbook
scripts/                 generator for the demo workbook
```

## Persistence

Local JSON files in the per-user application data directory
(`%APPDATA%\com.ourfault.desktop` on Windows; override with `OURFAULT_DATA_DIR`):

| File | Owner |
| --- | --- |
| `systems.json` | `JsonSystemRepository` |
| `settings.json` | local settings (e.g. highlight colour) |
| `mock-sharepoint/investigations.json` | `MockSharePoint` |
| `mock-mail/outbox.json` | `MockDistribution` |

Writes are atomic (temp file + rename). A corrupt file is reported, never
silently overwritten; the UI then shows a startup error instead of crashing.

JSON was chosen over SQLite because the data is small, human-auditable and
temporary: in production, systems and investigations move to shared storage
behind the same traits.

## Investigation numbers

`{running number}-{year}`, displayed with at least three digits (`001-2026`,
`056-2026`, `1000-2026`). The running number restarts each calendar year.

Uniqueness is **not** a UI concern:

1. The wizard shows the *expected* next number (`peek_next_investigation_number`),
   marked as automatically assigned.
2. On creation, the service asks the adapter to `create_investigation` with a
   candidate number. The adapter contract requires an **atomic create-if-absent**
   that fails with `NumberTaken`.
3. On `NumberTaken`, the service takes a fresh number from the backend and
   retries (bounded). The success screen explains if the number changed.

For real SharePoint this maps to a list column with *Enforce unique values*
(a duplicate insert fails and becomes `NumberTaken`), or an ETag-guarded counter
item. `services/investigations.rs` has a test that simulates another
workstation winning the race.

## Excel import

- `calamine` reads **cached cell values**. Formulas are not evaluated, and
  VBA/macros are never loaded. Only `.xlsx` is accepted.
- The header row is located by name within the first 10 rows (English or
  Hebrew titles: `Time/שעה`, `From/ממי`, `To/למי`, `Description/תוכן`,
  optional `Event type/סוג אירוע`), so title rows above the header are fine.
- Values become plain text: Excel times become `HH:MM`, control characters are
  stripped, cells are capped at 2,000 characters, and the sheet at 5,000 rows.
- **Background colour pre-selection**: calamine does not expose styles, so
  `adapters/excel/fills.rs` (~200 lines) reads `workbook.xml`, `styles.xml` and
  the first sheet via `zip` + `quick-xml`. Both crates are already calamine
  dependencies, so nothing new enters the tree. Only explicit RGB solid fills
  are matched (default `FFFF00`, configurable in `settings.json`).
  Theme/indexed colours and conditional formatting are *not* resolved; such rows
  are simply not pre-selected. Colour is only a starting point: the operator's
  checkbox selection is what counts, and a failure here never fails the import.
- The imported log stays in Rust memory. The wizard sends only row ids, so
  investigations are built from parsed data, not from data echoed back by the webview.

## Security boundaries

| Concern | Measure |
| --- | --- |
| Webview privileges | One capability (`capabilities/main-window.json`) granting only the 14 OurFault commands (generated in `build.rs`). No core, fs, dialog, shell, http or opener permissions. `withGlobalTauri: false`. |
| File access | The native file dialog is opened **by Rust**; the webview never supplies a path. Size limits apply before parsing, plus a zip-bomb guard on declared uncompressed size, and a per-part read cap in the fill reader. |
| Content injection | Excel values and user input are rendered as React text only; there is no `dangerouslySetInnerHTML` and no `eval`. |
| Network | No outbound requests anywhere. CSP `connect-src ipc: http://ipc.localhost`; verified in the real webview that `fetch` and `eval` are blocked. The navigation guard keeps the webview on the app origin. The preliminary-checks link is validated structurally (http/https, host, no credentials, ≤2048 chars), stored and shown with a *copy* button, never opened or fetched. |
| Authorisation | Admin commands check the role in Rust (`require_admin`), not only in the UI. PoC role source: `OURFAULT_DEMO_ROLE`. |
| Errors | Only codes reach the UI; paths and library errors go to the log. |
| Trust boundary validation | Every command input (draft, system configuration, numbers) is validated in the domain layer. |
| Supply chain | Small dependency set, pinned by `Cargo.lock` / `package-lock.json`. CI uses only GitHub-owned actions. |

## Decisions that deviate from the brief

- **Systems cannot be deleted.** Only deactivation exists. Investigations keep
  a snapshot of the system name and template, so history never dangles and
  later template edits never rewrite past investigations.
- **Links are copied, not opened.** Opening a pasted URL from the app would be
  the network access the brief rules out, and a phishing vector. Operators
  copy the link into their browser.
- **"פתח תחקיר" opens the in-app view** of the stored investigation. There is
  no real SharePoint document to open in the PoC.
- **Admin role from an environment variable.** A login screen would be
  throw-away work. The real source is the Windows identity plus a directory group.

## Replacing the mocks

1. Implement the trait (`SharePointAdapter`, `DistributionAdapter`,
   `SystemRepository`) in a new file under `src-tauri/src/adapters/`.
2. Construct it in `Backend::open` (`state.rs`).
3. Keep secrets (Graph tokens, SMTP credentials) in Rust, loaded from the OS
   credential store or managed configuration, never in the frontend or source.
4. If the adapter needs network access, perform it in Rust. The webview CSP
   stays closed.

Nothing in `services/`, `commands.rs` or the UI needs to change.

## Dependencies

| Dependency | Why |
| --- | --- |
| `tauri`, `tauri-build` | Desktop shell and IPC |
| `tauri-plugin-dialog` | Native "open file" dialog, used from Rust only |
| `calamine` | Pure-Rust `.xlsx` value reader, no macro execution |
| `zip`, `quick-xml` | Fill-colour detection (already transitive via calamine) |
| `chrono` | Local date/time (Windows-safe local offset) |
| `url` | URL parsing for validation (already transitive via Tauri) |
| `serde`, `serde_json`, `thiserror` | Serialisation and error types |
| `react`, `react-dom` | UI |
| `@tauri-apps/api` | Typed `invoke` |
| dev: `vite`, `@vitejs/plugin-react`, `typescript`, `vitest`, `@tauri-apps/cli` | Build and tests |

No UI kit, router, state library or icon package: the app has four screens,
and a discriminated union plus a reducer is clearer than a framework.

## Known limitations (PoC)

- The investigation is stored as structured data; rendering a `.docx` from a
  Word template belongs in a real SharePoint adapter.
- Frontend types in `src/api/types.ts` mirror the Rust serde types by hand.
  If the API grows, generate them (e.g. `ts-rs`).
- Internal logging is `stderr` only; add `tauri-plugin-log` with a rolling file
  before production.
- Light theme only.
