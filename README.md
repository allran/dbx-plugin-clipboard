# Clipboard History

Browse, search, and reuse local clipboard history inside DBX.

## Features

- Continuously monitors the system clipboard (text, images, and file lists) via a Rust sidecar
- Searchable history with tabs (All / Text / Image / Files)
- Click a row to preview; double-click to copy it back to the clipboard
- History persisted under `DBX_PLUGIN_DATA_DIR` (falls back to a temp directory in dev)

## Develop

Requires Node.js 22+ (see `.nvmrc`) and a Rust toolchain:

```bash
nvm use
npm install
dbx-plugin dev --path . --port 5190
```

UI source lives in `src/*.svelte` and builds into `ui/` via Vite.

## Package

```bash
dbx-plugin package .
```

Produces an unsigned platform-specific `.dbxp` under `dist/`.

## Notes

- Text, images, and file-path lists are tracked.
- Images are stored as PNG blobs under the plugin data directory (longest edge capped at 4096px).
- File history stores absolute paths only (not file contents); copying restores the path list to the clipboard.
- History is capped at 500 items / 100k characters per text item.
