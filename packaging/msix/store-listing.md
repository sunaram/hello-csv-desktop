# Microsoft Store listing (copy into Partner Center)

**Product name:** Hello CSV (if the name is taken, use "Hello CSV - Viewer & Editor"; whatever you reserve must match the
`-DisplayName` / identity values given to `build.ps1`)
**Privacy policy URL:** https://github.com/sunaram/hello-csv-desktop/blob/main/PRIVACY.md
**Support / website:** https://github.com/sunaram/hello-csv-desktop
**Category:** Productivity (or Developer tools)
**System requirements:** minimum 2 GB RAM, recommended 4 GB. Memory use stays low even for huge files (large files are read from disk, not loaded into memory).
**Age rating:** no user-generated content hosting, no online features - answer "No" to the questionnaire items.
**Pricing:** your choice; the Markdown viewer is $0.99 on the Store and free on GitHub.

## Short description (max 100 characters)

Fast CSV and TSV viewer and editor. Opens huge files, searches, sorts, edits. Tiny and private.

## Description (max 10,000 characters)

Hello CSV opens your CSV and TSV files instantly - even the ones that make spreadsheets freeze - and lets you search, sort and edit them. It is tiny, starts in a blink and keeps everything on your device.

OPENS FILES OF ANY SIZE
Most tools load the whole file into memory and slow to a crawl. Hello CSV reads big files straight from disk: the first rows appear immediately while the app indexes the rest in the background. Jump to any page, scroll through millions of rows and search the entire file in seconds. A 170 MB file with two million rows opens right away.

INSTANT SEARCH
Type in the search box to filter rows and see every match highlighted. On large files results stream in as the file is scanned, so you never wait for the search to finish before you start reading.

EDIT LIKE A SPREADSHEET
Double-click a cell, press Enter or F2, or simply start typing. A value bar at the top shows and edits the full contents of long cells. Move with the arrow keys and Tab, undo and redo, insert and delete rows, add columns and rename headers. Changed cells are marked so you always see what you edited. Save with Ctrl+S: Hello CSV writes valid CSV, keeps your delimiter, line endings and encoding marker, and on huge files rewrites only the rows you changed, so saving is fast and every other byte stays exactly as it was.

EXPLORE YOUR DATA
- Click a column header to sort (numbers sort as numbers, text sorts naturally)
- Sticky header and row numbers stay in view while you scroll
- Pages of 25 to 10,000 rows, with first / previous / next / last
- Delimiter detected automatically (comma, semicolon, tab, pipe) or chosen by you
- Treat the first row as column names, or not
- Copy the current view as a Markdown table or as tab-separated text, ready to paste into Excel, Google Sheets, an issue or a README

BUILT FOR EVERYDAY USE
- Open files from the File menu, by drag and drop, or by double-clicking a .csv, .tsv or .tab file
- Live reload: the view refreshes when the file changes on disk (unless you have unsaved edits)
- Recent files list, light / dark / system theme and zoom, all remembered
- Keyboard friendly: Ctrl+O open, Ctrl+S save, Ctrl+F search, Ctrl+Z / Ctrl+Y undo and redo

SMALL AND PRIVATE
The app is under 1 MB. It has no account, no ads, no analytics and no telemetry, and your files never leave your computer. Hello CSV is open source (MIT): https://github.com/sunaram/hello-csv-desktop

## What's new in this version (max 1,500 characters)

First release of Hello CSV for Windows.
- Opens CSV, TSV and other delimited files of any size, with instant first paint
- Fast search with highlighted matches, even across millions of rows
- Edit cells in place, add or delete rows, add columns, rename headers, undo and redo
- Sort, page through the data, copy as Markdown or TSV
- Light and dark themes, recent files, live reload

## Product features (max 20 entries, 200 characters each)

1. Opens huge CSV and TSV files instantly: big files are read from disk, not loaded into memory
2. Search across millions of rows with highlighted matches; results stream in while the file is scanned
3. Edit cells in place: double-click, Enter, F2, or just start typing; value bar for long text
4. Undo and redo, insert and delete rows, add columns, rename headers
5. Save writes valid CSV and keeps your delimiter, line endings and encoding marker
6. On large files, saving rewrites only the rows you changed - fast, with constant memory use
7. Click-to-sort columns with numeric-aware ordering
8. Sticky header and row numbers; pages from 25 to 10,000 rows
9. Automatic delimiter detection (comma, semicolon, tab, pipe) with manual override
10. Toggle whether the first row is a header
11. Copy the current view as a Markdown table or as TSV for Excel and Google Sheets
12. Modified cells are marked, and a clear Unsaved indicator shows pending changes
13. Open by menu, drag and drop, or double-clicking .csv, .tsv and .tab files; recent files list
14. Live reload when the file changes on disk
15. Light, dark and system themes; zoom; settings remembered
16. Under 1 MB, instant start-up; no account, no ads, no telemetry - your files stay on your device

## Search terms (max 7)

csv, tsv, viewer, editor, spreadsheet, table, data

## Screenshots (1920 x 1080, in `docs/screenshots/`)

| File | Suggested caption |
|---|---|
| `table.png` | Sort, select and inspect any cell in a clean table |
| `editing.png` | Edit cells in place; changes are marked until you save |
| `search-dark.png` | Instant search with highlighted matches (dark theme) |
| `large-file.png` | Two million rows open instantly (dark theme) |
| `large-search.png` | Search a 170 MB file in seconds, results stream in |

## Store display images (`docs/store/`, generated by `packaging/msix/gen-store-art.ps1`)

| Partner Center slot | File | Size |
|---|---|---|
| 9:16 Poster art | `poster-720x1080.png` | 720 x 1080 |
| 1:1 Box art | `box-art-1080x1080.png` | 1080 x 1080 |
| 1:1 Store logo (small) | `box-art-300x300.png` | 300 x 300 |
| 16:9 Super hero art (optional) | `hero-1920x1080.png` | 1920 x 1080 |

## Notes

- Requires the WebView2 runtime (preinstalled on Windows 11, normally present on Windows 10).
- The package logos (`packaging/msix/Assets/*.png`) are generated from `assets/icon.png` by `gen-icons.ps1`.
- Re-create the screenshots with `tools/screenshot.ps1` (see its header); sample data comes from `tools/gen-sample.mjs`.
- Before uploading: build with the identity values from Partner Center: `packaging/msix/build.ps1 -Identity ... -Publisher ... -PublisherDisplayName ... -DisplayName ...`.
