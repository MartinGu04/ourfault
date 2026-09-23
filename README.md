# OurFault

Windows desktop application that turns rows of an operations-log Excel workbook
into an investigation report, without copying rows by hand.

**בחר שורות → בחר מערכת → הדבק קישור → בדוק → צור**

The UI is Hebrew and right-to-left. Code, comments and documentation are in
English. This is a proof of concept: Excel reading is real, while SharePoint
and e-mail distribution are local mocks. All bundled data is fictional.

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

1. **תחקיר חדש** → **שימוש בקובץ הדגמה**, or pick
   [`demo/operations-log-demo.xlsx`](demo/operations-log-demo.xlsx) with
   **בחירת קובץ Excel…**. The six rows filled yellow in Excel are pre-selected.
   Adjust the selection as needed.
2. **המשך** → choose *מערכת אלפא*, paste e.g. `https://checks.example.com/runs/4471`.
   The number (`056-2026`) and the template come from the backend and the system.
3. **תצוגה מקדימה** → **צור תחקיר**. The investigation is saved by the mock
   SharePoint adapter.
4. **הפץ במייל** shows the exact message and the system's distribution list,
   then "sends" it to a local outbox.
5. **ניהול מערכות** (home screen, administrators): create or edit systems,
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
cargo test                 # domain, adapters, services, end-to-end backend workflow
cargo clippy --all-targets -- -D warnings

npm run tauri build        # Windows installers (NSIS + MSI) in src-tauri/target/release/bundle
```

CI (`.github/workflows/ci.yml`) runs all checks on Windows and publishes the
NSIS installer as a build artifact.

## Project layout

```
src/            React UI (Hebrew, RTL) – screens under src/features, IPC client in src/api
src-tauri/src/  Rust: domain rules, services, adapters (Excel, mock SharePoint, mock mail), commands
src-tauri/seed/ fictional first-run data
demo/           fictional operations-log workbook (regenerate: python scripts/generate_demo_workbook.py)
docs/           architecture and security notes
```
