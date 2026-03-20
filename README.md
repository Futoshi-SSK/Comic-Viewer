# Comic Viewer

A desktop comic and manga reader built with Tauri + SvelteKit + Rust.

## Features

- **Supported formats**: ZIP, CBZ, PDF
- **Nested ZIP support**: Reads archives within archives
- **Spread view**: Side-by-side page display with RTL (right-to-left) support for manga
- **Single page view**: Toggle between spread and single page mode
- **Reading position memory**: Automatically saves and restores your last position per file
- **Drag & drop**: Drop a file directly onto the window to open it
- **Multiple navigation methods**: Keyboard, mouse click, and scroll wheel

## Navigation

| Action | Result |
|--------|--------|
| `←` / `Space` | Next page |
| `→` | Previous page |
| Click left half | Next page |
| Click right half | Previous page |
| Mouse wheel | Scroll through pages |

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install) + Cargo
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) (Windows)

## Development

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

The installer will be output to `src-tauri/target/release/bundle/`.

## Tech Stack

- **Frontend**: SvelteKit 5, TypeScript, Vite
- **Backend**: Tauri 2, Rust
- **PDF rendering**: pdf.js
- **ZIP handling**: zip crate (Rust)

## Code Signing

Releases are signed with [SignPath Foundation](https://signpath.io/product/foundation).
Signatures can be verified at [sig.fo/Comic-Viewer-Futoshi-SSK](https://sig.fo/Comic-Viewer-Futoshi-SSK).

## License

MIT
