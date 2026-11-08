# DxTool

A modern, free and open-source diagnostic and repair tool for Windows 10 and 11 PCs.

Built for regular people, with expert-level detail one click away.

- **What it does:** see [FEATURES.md](FEATURES.md)
- **What changed:** see [CHANGES.md](CHANGES.md)
- **Original requirements:** [docs/Requirements-Scaffold.docx](docs/Requirements-Scaffold.docx)

## Download

Get the latest **DxTool.exe** (portable, no install) or the **installer** from the [Releases page](https://github.com/iandscofield/DxTool/releases/latest). DxTool can update itself from Settings → Updates.

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

## Making a release

1. Set the new version in `Cargo.toml`, `package.json` and `src-tauri/tauri.conf.json`, and add a `## Version x.y.z · …` section to CHANGES.md.
2. Commit, then push a tag: `git tag v0.6.0` and `git push origin v0.6.0`.
3. GitHub Actions (`.github/workflows/release.yml`) builds DxTool, the installer and `SHA256SUMS.txt`, and publishes the release with that CHANGES.md section as its notes.

Quick engine test without the interface:

```bash
cargo run -p dxcore --example inventory
cargo run -p dxcore --example scan          # Fast scan (or: full, or a list of check ids)
```

## Licence

MIT. See [LICENSE](LICENSE).
