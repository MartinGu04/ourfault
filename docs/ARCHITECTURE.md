# OurFault – Architecture

This document explains how the PoC is put together and why. It is meant to be
read before changing the code.

## Goals and non-goals

OurFault creates investigations from operations-log rows:

**Select rows in Excel → Copy → Paste into OurFault → Review → choose system → paste checks link → preview → create**

- **Input is operator-controlled.** The operator selects and copies the rows
  in Excel. OurFault never opens or reads a workbook, and never infers,
  scores or auto-selects rows. It parses exactly what was pasted, deterministically,
  and the operator reviews every row (edit, remove, paste more) before continuing.
- **SharePoint is the destination and the source of truth.** An investigation
  is created as an editable SharePoint list item (web form). Once it exists,
  it is read and edited in SharePoint. OurFault keeps no copy of its own and
  exports no PDF or DOCX.
- One desktop process (Tauri 2). No web server, no background services.
- SharePoint and e-mail are **mocks** that write local JSON files and never
  touch the network. All data is fictional.

## Layers

```
React UI (src/)                       Hebrew, RTL, presentation and wizard state only
   │  typed calls in src/api/client.ts
   ▼  Tauri IPC (allow-listed commands)
commands.rs                           input/output translation, auth checks
   ▼
services/                             use cases: numbering, create, distribute, admin
   ▼
domain/                               pure types and rules, no I/O (incl. paste parsing)
   ▲
adapters/  (implement traits in adapters/mod.rs)
   ├─ mock_sharepoint.rs     SharePointAdapter   → local JSON "list"
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
| Parsing pasted rows, validating reviewed rows | `domain/log_rows.rs` |
| Number format, parsing, incrementing | `domain/investigation_number.rs` |
| Link, e-mail and text validation | `domain/validation.rs` |
| Building an investigation (active system, rows, link, template snapshot) | `domain/investigation.rs` |
| System configuration validation | `domain/system.rs` |
| Number allocation with conflict retry | `services/investigations.rs` |
| Distribution message content | `domain/distribution.rs` |

Errors cross IPC only as stable codes (`{kind: "validation", errors: [{field, code}]}`,
`{kind: "paste", code}`). The UI maps codes to Hebrew text in
`src/api/errors.ts`. Technical details stay in the log (`log_internal`).

## Project structure

```
src/                     React + TypeScript (strict)
  api/                   IPC types, the typed client, error → Hebrew messages
  features/home          home screen: new investigation, search, recent list
  features/wizard        paste & review, details, preview, success
  features/investigation shared investigation view, mock SharePoint item screen, distribution dialog
  features/admin         system administration
  ui/                    small shared controls (buttons, fields, dialog, icons)
  styles/                design tokens and CSS (logical properties → native RTL)
src-tauri/
  src/                   Rust application (see layers above)
  seed/                  fictional systems and past investigations (first-run seed)
  capabilities/          the single Tauri capability (explicit command allow-list)
demo/                    fictional operations log to copy from + matching paste text
scripts/                 generator for the demo files
```

## Pasting rows

When Excel copies a selection, it puts it on the clipboard as tab-separated
text: CRLF between rows, and quotes around cells that contain line breaks.

1. The UI listens for the `paste` event on the first wizard step. It uses the
   clipboard's plain-text data only, and only when the operator pastes. The
   app has **no clipboard permission** and never reads the clipboard on its own.
2. The text goes to the `parse_pasted_rows` command. `domain/log_rows.rs`:
   - splits rows and cells, honouring Excel's quoting for multi-line cells;
   - drops completely blank lines;
   - if the first line holds the column titles (English or Hebrew, any order),
     uses it to locate the columns and skips it; otherwise takes columns **by
     position** in the log's order: Time, From, To, Description. Further
     columns (such as Event type) are not part of an investigation;
   - rejects pastes that are empty, too large (1 MB), have fewer than four
     columns, more than 500 rows, or cells over 2,000 characters;
   - removes control characters. Values are otherwise kept exactly as pasted.
3. The rows appear in a review table. The operator can remove rows, edit a
   row, paste more rows (which are appended) or clear everything. Nothing is
   reordered or filtered automatically.
4. On preview and create, the reviewed rows are sent with the draft and
   validated again in Rust (`validate_rows`).

The Excel reading and fill-colour detection from the first iteration have
been removed completely, along with the file dialog.

## SharePoint model

- Each system has a SharePoint destination: a site URL and a **list**.
  Investigations are items in that list, and each item's form is where the
  investigation is completed and edited.
- `SharePointAdapter::create_investigation` creates the item and returns it
  with its item id and URL (the mock returns
  `{site}/Lists/{list}/DispForm.aspx?ID={id}`).
- After creation, every OurFault view (recent list, search, the
  "פתח תחקיר" screen, distribution) reads back from the adapter. In the PoC,
  "פתח תחקיר" shows the mock item inside the app, since the URL is fictional.
  With a real adapter it should open the item in SharePoint (see below).
- The investigation carries a snapshot of the system name and template, so
  later configuration changes never alter existing investigations.

## Persistence

Local JSON files in the per-user application data directory
(`%APPDATA%\com.ourfault.desktop` on Windows; override with `OURFAULT_DATA_DIR`):

| File | Owner |
| --- | --- |
| `systems.json` | `JsonSystemRepository` |
| `mock-sharepoint/investigations.json` | `MockSharePoint` (stands in for the SharePoint list) |
| `mock-mail/outbox.json` | `MockDistribution` |

Writes are atomic (temp file + rename). A corrupt file is reported, never
silently overwritten; the UI then shows a startup error instead of crashing.
In production, system configuration moves to shared storage behind the same
trait, and investigations live only in SharePoint.

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

## Security boundaries

| Concern | Measure |
| --- | --- |
| Webview privileges | One capability (`capabilities/main-window.json`) granting only the 13 OurFault commands (generated in `build.rs`). No core, fs, dialog, clipboard, shell, http or opener permissions. `withGlobalTauri: false`. |
| Input | Pasted text is untrusted. It is bounded in size, rows and cell length, and cleaned of control characters. Reviewed rows are re-validated in Rust on every preview and create. Admin configuration is validated in the domain layer. |
| File system | The webview has no file access and never supplies paths; only Rust writes the app's own data files. |
| Content injection | Pasted values and user input are rendered as React text only; there is no `dangerouslySetInnerHTML` and no `eval`. |
| Network | No outbound requests anywhere. CSP `connect-src ipc: http://ipc.localhost`; verified in the real webview that `fetch` and `eval` are blocked. The navigation guard keeps the webview on the app origin. The preliminary-checks link is validated structurally (http/https, host, no credentials, ≤2048 chars), stored and shown with a *copy* button, never opened or fetched. |
| Authorisation | Admin commands check the role in Rust (`require_admin`), not only in the UI. PoC role source: `OURFAULT_DEMO_ROLE`. |
| Errors | Only codes reach the UI; paths and library errors go to the log. |
| Supply chain | Small dependency set, pinned by `Cargo.lock` / `package-lock.json`. CI uses only GitHub-owned actions. |

## Decisions that deviate from the brief

- **Systems cannot be deleted.** Only deactivation exists. Investigations keep
  a snapshot of the system name and template, so history never dangles and
  later template edits never rewrite past investigations.
- **The preliminary-checks link is copied, not opened.** Opening an arbitrary
  pasted URL from the app would be the network access the brief rules out.
- **Admin role from an environment variable.** A login screen would be
  throw-away work. The real source is the Windows identity plus a directory group.

## Replacing the mocks

1. Implement the trait (`SharePointAdapter`, `DistributionAdapter`,
   `SystemRepository`) in a new file under `src-tauri/src/adapters/`.
   For SharePoint (e.g. via Microsoft Graph), `create_investigation` creates
   the list item, maps the unique-number conflict to `NumberTaken`, and
   returns the item's id and web URL.
2. Construct it in `Backend::open` (`state.rs`).
3. Keep secrets (Graph tokens, SMTP credentials) in Rust, loaded from the OS
   credential store or managed configuration, never in the frontend or source.
4. Perform network access in Rust; the webview CSP stays closed.
5. To open an item in SharePoint, add a narrow Rust command that opens a URL
   **only if it belongs to a configured SharePoint site** (an allowlist taken
   from system configuration). Do not add a general "open URL" capability.

Nothing in `services/`, `commands.rs` or the UI needs to change for 1–4.

## Dependencies

| Dependency | Why |
| --- | --- |
| `tauri`, `tauri-build` | Desktop shell and IPC |
| `chrono` | Local date/time (Windows-safe local offset) |
| `url` | URL parsing for validation (already transitive via Tauri) |
| `serde`, `serde_json`, `thiserror` | Serialisation and error types |
| `react`, `react-dom` | UI |
| `@tauri-apps/api` | Typed `invoke` |
| dev: `vite`, `@vitejs/plugin-react`, `typescript`, `vitest`, `@tauri-apps/cli` | Build and tests |

No UI kit, router, state library, icon package, spreadsheet library or
clipboard plugin. The app has four screens, and a discriminated union plus a
reducer is clearer than a framework.

## Known limitations (PoC)

- Frontend types in `src/api/types.ts` mirror the Rust serde types by hand.
  If the API grows, generate them (e.g. `ts-rs`).
- Columns are mapped by position unless a header row is pasted. If the real
  log's column order differs, change `DEFAULT_COLUMNS` in `domain/log_rows.rs`,
  or make it part of the configuration.
- Internal logging is `stderr` only; add `tauri-plugin-log` with a rolling file
  before production.
- Light theme only.
