# overview - PLOT3D Viewer

[![Build and Release](https://github.com/cwj5/overview/actions/workflows/build.yml/badge.svg)](https://github.com/cwj5/overview/actions/workflows/build.yml)
[![Test and Coverage](https://github.com/cwj5/overview/actions/workflows/test-coverage.yml/badge.svg)](https://github.com/cwj5/overview/actions/workflows/test-coverage.yml)
[![TypeScript Tests](https://img.shields.io/badge/TypeScript_Tests-100%2F100-brightgreen)](https://github.com/cwj5/overview)
[![TypeScript Coverage](https://img.shields.io/badge/TypeScript_Coverage-97.62%25-brightgreen)](https://github.com/cwj5/overview)
[![Rust Tests](https://img.shields.io/badge/Rust_Tests-86%2F86-brightgreen)](https://github.com/cwj5/overview)
[![Rust Coverage](https://img.shields.io/badge/Rust_Coverage-45.28%25-yellow)](https://github.com/cwj5/overview)

[![Latest Linux Artifact](https://img.shields.io/badge/Linux-Download%20AppImage-blue)](https://github.com/cwj5/overview/actions/workflows/build.yml?query=branch%3Amain+is%3Asuccess)
[![Latest Windows Artifact](https://img.shields.io/badge/Windows-Download%20MSI%2FNSIS-blue)](https://github.com/cwj5/overview/actions/workflows/build.yml?query=branch%3Amain+is%3Asuccess)
[![Latest macOS Artifact](https://img.shields.io/badge/macOS-Download%20App-blue)](https://github.com/cwj5/overview/actions/workflows/build.yml?query=branch%3Amain+is%3Asuccess)

A modern, cross-platform application for visualizing CFD (Computational Fluid Dynamics) grid and solution data in PLOT3D format.

## Features

- **PLOT3D File Support**: Read and parse PLOT3D binary grid files
- **3D Visualization**: Interactive 3D rendering using Three.js
- **Wireframe & Shaded Modes**: Toggle between wireframe and flat-shaded rendering
- **Multi-Grid Support**: Handle multiple computational grids
- **Cross-Platform**: Runs on Linux, Windows, and macOS

## Tech Stack

- **Frontend**: React + TypeScript + Three.js
- **Backend**: Rust (via Tauri)
- **3D Rendering**: React Three Fiber + Drei
- **Desktop Framework**: Tauri 2.0

## Prerequisites

- Node.js (v20 or later)
- Rust (latest stable)
- npm

### System Requirements for Pre-built Binaries

**macOS**: 
- macOS 11.0 or later
- Intel (x86_64) or Apple Silicon (aarch64)

**Linux**:
- glibc 2.35 or later (Ubuntu 22.04+, Fedora 36+, Debian 12+, Rocky Linux 9+)
- AppImage format (no installation required, just make executable and run)

**Windows**:
- Windows 10 or later
- MSI installer

## Getting Started

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## Headless CLI Export (TKT-011 Phase 1)

For backnode and automation workflows, a standalone Rust CLI exporter is now available.

Key properties:

- No browser, WebGL, or desktop window is required.
- Rendering uses pure Rust crates (`clap` and `image`) to avoid unusual system-library requirements.
- Multi-`PLOT` scripts emit numbered outputs (`_001`, `_002`, ...).

Run from repo root:

```bash
cargo run --manifest-path headless-export/Cargo.toml --bin overview-export -- \
	--cmd path/to/script.com \
	--out path/to/output.png
```

Notes:

- If the script emits one `PLOT`, `--out` is used exactly.
- If the script emits multiple `PLOT` intents, output files are suffixed automatically.
- This is a bootstrap headless renderer for deterministic automation. Visual output is not yet fully equivalent to the in-app Three.js export path and is tracked under TKT-011.

## Testing

This project maintains high code quality with comprehensive automated tests:

### Running Tests

```bash
# Run all TypeScript tests
npm test

# Run TypeScript tests with coverage report
npm run test:coverage

# Watch mode for TypeScript tests
npm run test:watch

# Run Rust library tests
cd src-tauri && cargo test --lib

# Run headless CLI crate tests
cargo test --manifest-path headless-export/Cargo.toml --bin overview-export

# Smoke-run headless CLI export fixture
cargo run --manifest-path headless-export/Cargo.toml --bin overview-export -- \
  --cmd headless-export/fixtures/smoke.com \
  --out /tmp/overview-cli-smoke/smoke.png \
  --width 320 \
  --height 200

# Generate Rust coverage report
cd src-tauri && cargo tarpaulin --lib --timeout 300
```

### Pre-commit Hooks

Tests are automatically run before each commit to ensure code quality:

```bash
# The hooks are configured during development setup
# To bypass hooks (not recommended): git commit --no-verify
```

### Coverage Status

- **TypeScript**: 97.62% coverage (100 tests)
- **Rust**: 45.28% coverage (86 tests)
- **Total**: 186 tests across full stack

Coverage reports are generated automatically in GitHub Actions on all pull requests.

## Project Structure

```
mehu/
├── src/                    # Frontend React/TypeScript code
│   ├── components/         # React components
│   │   └── Viewer3D.tsx   # Main 3D viewer component
│   ├── App.tsx            # Main app component
│   └── main.tsx           # Entry point
├── src-tauri/             # Rust backend code
│   ├── src/
│   │   ├── lib.rs         # Main Tauri application
│   │   └── plot3d.rs      # PLOT3D file parser
│   └── Cargo.toml         # Rust dependencies
└── package.json           # Node.js dependencies
```

## Architecture

**Frontend (React + Three.js)**:
- Handles UI and 3D visualization
- Lightweight, focuses on rendering only
- Uses React Three Fiber for declarative 3D scenes

**Backend (Rust)**:
- Parses PLOT3D binary files efficiently
- Manages large mesh data (million+ points)
- Provides Tauri commands for file operations

## Building for Distribution

Automated builds are handled via GitHub Actions. Binaries are built for:
- **macOS**: Both Intel and Apple Silicon architectures in one .app bundle
- **Linux**: AppImage (no installation needed, portable executable)
- **Windows**: MSI installer

Builds run automatically on push to `main` branch and tagged releases. Artifacts are available in the GitHub Actions tab.

Quick access to the latest downloadable artifacts:
- Linux AppImage: https://github.com/cwj5/overview/actions/workflows/build.yml?query=branch%3Amain+is%3Asuccess
- Windows MSI/NSIS: https://github.com/cwj5/overview/actions/workflows/build.yml?query=branch%3Amain+is%3Asuccess
- macOS app bundles: https://github.com/cwj5/overview/actions/workflows/build.yml?query=branch%3Amain+is%3Asuccess

PLOT3D is a NASA-developed format for storing CFD grid and solution data. For more information, see the [PLOT3D manual](https://ntrs.nasa.gov/api/citations/19900013774/downloads/19900013774.pdf).

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
