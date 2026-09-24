# OurFault

Windows desktop application for activity investigations. An investigation
belongs to an **activity** (mission, experiment, training or other) that
involves one or more systems. The operator fills in the activity details and
the administrator-configured technical sections, pastes the relevant
operations-log rows copied in Excel, adds the preliminary-checks link, and
creates the investigation. It is then distributed by e-mail to everyone on
the involved systems' distribution lists.

**Activity details → Technical sections → Paste event chronology from Excel → Checks link & review → Create → Distribute**

Work is saved automatically as a **draft** (without an investigation number)
and can be continued later. The final number (`056-2026`, `1000-2026`, …) is
allocated only when the investigation is created.

The UI is Hebrew and right-to-left. Code, comments and documentation are in
English. This is a proof of concept: SharePoint and e-mail are local mocks,
and all bundled data is fictional.

Stack: Tauri 2 · Rust · React · TypeScript · Vite. See
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the model, the boundaries and
how to replace the mocks.

## Run it

Prerequisites: Node.js 20+, Rust 1.88+ (stable), and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On Windows
10/11 that is the MSVC build tools; WebView2 is already installed.

```sh
npm ci
npm run tauri dev
```

On first start the app seeds fictional data: four systems (one deactivated),
four satellite stations, three technical sections, the distribution template,
past investigations up to `055-2026` (completed and distributed, every
activity type) and one saved draft (*בלט רומני*).

### Demo walkthrough

1. **Entry screen**: choose **כניסה רגילה**. (This chooses a work mode; it is
   not a login.)
2. **Home**: continue the draft *בלט רומני* under **טיוטות**, or start
   **תחקיר חדש**. Every change is saved automatically (*נשמר* at the top).
   Use **שמירה ויציאה** at any time and reopen the draft later.
3. **פרטי הפעילות**: name, type (choosing *אחר* asks for a description),
   one or more systems, activity status, planned and actual times, and the
   yes/no questions.
4. **פרטים טכניים**: the configured sections. Add and remove rows in the
   table sections; station fields list the configured stations.
5. **השתלשלות אירועים**: open [`demo/operations-log-demo.xlsx`](demo/operations-log-demo.xlsx)
   in Excel, select rows, copy, and press <kbd>Ctrl+V</kbd> in the app (or use
   **הדבקת שורות לדוגמה**). Review, edit or remove rows; paste more groups
   if needed.
6. **בדיקות מקדימות וסקירה**: paste e.g. `https://checks.example.com/runs/4480`.
   The page lists anything still missing (each item links to its step) and
   previews the document with the expected number. **ייצוא טיוטה ל-PDF** asks
   for confirmation and produces a file marked *טיוטה — לא להפצה*.
7. **יצירת תחקיר**: the number (`056-2026`) is allocated and the
   investigation is created in the mock SharePoint list. The draft leaves the
   drafts list.
8. **הפצה במייל** shows the message built from the template, addressed to
   the union of the systems' lists, and "sends" it to a local outbox. The
   status becomes *הופץ*. **ייצוא ל-PDF** writes the investigation to
   `Downloads\OurFault`.
9. **החלפת מצב עבודה** → **מצב מנהל**: the setup checklist, systems and
   distribution lists, stations, the technical-section builder, and the mail
   template and publication target. Restart the app: everything is still there.

### Useful environment variables

| Variable | Effect |
| --- | --- |
| `OURFAULT_DATA_DIR=<dir>` | Use another data directory, e.g. for a clean demo. Default: `%APPDATA%\com.ourfault.desktop`. |
| `OURFAULT_EXPORT_DIR=<dir>` | Where PDF exports go. Default: `Downloads\OurFault`. |
| `OURFAULT_ADMIN_MODE=disabled` | Hide admin mode (the backend refuses it too). |
| `OURFAULT_PUBLISHER=folder` + `OURFAULT_PUBLISH_DIR=<existing dir>` | Publish to a folder (`<dir>\2026\056-2026 - name.pdf` + JSON record) instead of the SharePoint mock. |

Delete the data directory to reset to the seed data. Data written by the
first PoC version (PR #1) is not migrated; see
[Local data and migration](docs/ARCHITECTURE.md#local-data-and-migration).

## Test and build

```sh
npm run typecheck          # strict TypeScript
npm test                   # frontend unit tests (Vitest)
cd src-tauri
cargo test                 # domain, rendering, adapters, services, end-to-end backend workflow
cargo clippy --all-targets -- -D warnings

npm run tauri build        # Windows installers (NSIS + MSI) in src-tauri/target/release/bundle
```

`SAMPLE_DIR=<dir> cargo test write_samples -- --ignored` writes a sample
completed and draft PDF for a visual check.

CI (`.github/workflows/ci.yml`) runs all checks on Windows and publishes the
NSIS installer as a build artifact.

## Project layout

```
src/                React UI (Hebrew, RTL) – screens under src/features, IPC client in src/api
src-tauri/src/      Rust: domain rules, rendering (HTML/PDF), services, adapters (mocks), commands
src-tauri/seed/     fictional first-run data
src-tauri/assets/   bundled font for PDF export (Alef, SIL Open Font License)
demo/               fictional operations log to copy from, plus the matching paste text
                    (regenerate both: python scripts/generate_demo_workbook.py)
docs/               architecture and security notes
```
