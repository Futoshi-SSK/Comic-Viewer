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

## Supported Platforms

Comic Viewer supports the following Windows architectures:

| Architecture | Rust Target | Description |
|---|---|---|
| x86 (32-bit) | `i686-pc-windows-msvc` | 32-bit Intel/AMD |
| x64 (64-bit) | `x86_64-pc-windows-msvc` | 64-bit Intel/AMD |
| ARM64 | `aarch64-pc-windows-msvc` | Windows on ARM |

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install) + Cargo
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) (Windows) — include the **Desktop development with C++** workload

## Switching the Build Target

The default build target is defined in `src-tauri/.cargo/config.toml`:

```toml
[build]
target = "i686-pc-windows-msvc"
```

Change the `target` value to build for a different architecture:

| Architecture | Target value |
|---|---|
| x86 (32-bit) | `i686-pc-windows-msvc` |
| x64 (64-bit) | `x86_64-pc-windows-msvc` |
| ARM64 | `aarch64-pc-windows-msvc` |

You also need to install the corresponding Rust target if not already present:

```bash
rustup target add i686-pc-windows-msvc     # x86
rustup target add x86_64-pc-windows-msvc   # x64
rustup target add aarch64-pc-windows-msvc  # ARM64
```

Alternatively, you can override the target at build time without editing the file:

```bash
npm run tauri build -- --target x86_64-pc-windows-msvc
```

## Development

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

The installer will be output to `src-tauri/target/<target>/release/bundle/`.
For example, an x86 build produces:
```
src-tauri/target/i686-pc-windows-msvc/release/bundle/
  msi/  comic-viewer_x.y.z_x86_en-US.msi
  nsis/ comic-viewer_x.y.z_x86-setup.exe
```

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
