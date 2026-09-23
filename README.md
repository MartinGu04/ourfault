# OurFault

Windows desktop application that creates investigations from operations-log
rows. The operator copies the relevant rows in Excel and pastes them into
OurFault in one go, instead of copying them into a template one by one.

**Select rows in Excel → Copy → Paste into OurFault → Review → Choose system → Paste checks link → Preview → Create**

The investigation is created in SharePoint as an editable list item (web
form), and from then on SharePoint is the source of truth. OurFault does not
export PDF or DOCX files.

The UI is Hebrew and right-to-left. Code, comments and documentation are in
English. This is a proof of concept: SharePoint and e-mail distribution are
local mocks, and all bundled data is fictional.

Stack: Tauri 2 · Rust · React · TypeScript · Vite. See
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for design, security boundaries
and how to replace the mocks.

## Run it

Prerequisites: Node.js 20+, Rust 1.88+ (stable), and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On Windows
10/11 that is the MSVC build tools; WebView2 is already installed.

```sh
npm ci
npm run tauri dev
```

On first start the app seeds fictional data: systems *מערכת אלפא* and
*מערכת בראבו* (active), *מערכת צ'ארלי* (deactivated, with history), and past
investigations up to `055-2026`.

### Demo walkthrough

1. Open [`demo/operations-log-demo.xlsx`](demo/operations-log-demo.xlsx) in
   Excel, select rows (for example the yellow incident rows, or the whole
   08:14–10:02 block) and copy them. The yellow fill is only a visual aid
   for the demo; OurFault ignores formatting.
2. **תחקיר חדש** → click the paste box and press <kbd>Ctrl+V</kbd>. Without
   Excel, use **הדבקת שורות לדוגמה**. Review the rows: remove unrelated ones,
   fix a row with the edit button, or paste more rows to add them.
3. **המשך** → choose *מערכת אלפא*, paste e.g. `https://checks.example.com/runs/4471`.
   The number (`056-2026`) and the template come from the backend and the system.
4. **תצוגה מקדימה** → **צור תחקיר**. The mock SharePoint adapter creates the
   investigation item and returns its (fictional) URL, for example
   `…/Lists/Investigations/DispForm.aspx?ID=47`.
5. **הפץ במייל** shows the exact message and the system's distribution list,
   then "sends" it to a local outbox.
6. **ניהול מערכות** (home screen, administrators): create or edit systems,
   templates and distribution lists, or deactivate a system. Restart the app:
   everything is still there.

### Useful environment variables

| Variable | Effect |
| --- | --- |
| `OURFAULT_DEMO_ROLE=operator` | Run as a non-admin operator (default: admin). Admin commands are refused by the backend. |
| `OURFAULT_DATA_DIR=<dir>` | Use another data directory, e.g. for a clean demo. Default: `%APPDATA%\com.ourfault.desktop`. |

Delete the data directory to reset to the seed data.

## Test and build

```sh
npm run typecheck          # strict TypeScript
npm test                   # frontend unit tests (Vitest)
cd src-tauri
cargo test                 # paste parsing, domain, adapters, services, end-to-end backend workflow
cargo clippy --all-targets -- -D warnings

npm run tauri build        # Windows installers (NSIS + MSI) in src-tauri/target/release/bundle
```

CI (`.github/workflows/ci.yml`) runs all checks on Windows and publishes the
NSIS installer as a build artifact.

## Project layout

```
src/            React UI (Hebrew, RTL) – screens under src/features, IPC client in src/api
src-tauri/src/  Rust: domain rules (incl. paste parsing), services, adapters (mock SharePoint, mock mail), commands
src-tauri/seed/ fictional first-run data
demo/           fictional operations log to copy from, plus the matching paste text
                (regenerate both: python scripts/generate_demo_workbook.py)
docs/           architecture and security notes
```
