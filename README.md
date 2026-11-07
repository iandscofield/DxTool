# DxTool

A modern, free and open-source diagnostic and repair tool for Windows 10 and 11 PCs.

Built for regular people, with expert-level detail one click away.

- **What it does:** see [FEATURES.md](FEATURES.md)
- **What changed:** see [CHANGES.md](CHANGES.md)
- **Original requirements:** [docs/Requirements-Scaffold.docx](docs/Requirements-Scaffold.docx)

## Project layout

| Folder | What's in it |
|---|---|
| `crates/dxcore` | The diagnostic engine (Rust). Reads hardware, runs checks. No user interface code. |
| `src-tauri` | The desktop app shell (Rust + Tauri). Connects the engine to the window. |
| `src` | The user interface (React + TypeScript). |
| `docs` | Requirements and design notes. |

## Building from source

Requirements: Windows 10/11, [Rust](https://rustup.rs), [Node.js](https://nodejs.org) 20+, and the Visual Studio C++ Build Tools.

```bash
npm install
npm run tauri dev      # run the app in development mode
npm run tauri build    # build DxTool.exe and the installer
```

Quick engine test without the interface:

```bash
cargo run -p dxcore --example inventory
```

## Licence

MIT. See [LICENSE](LICENSE).
