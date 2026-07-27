# Repository Guidelines

## Project Structure

This directory is the production Windows desktop application. Vue 3 and TypeScript code lives in `src/`: pages are in `views/`, reusable UI in `components/`, Pinia state in `stores/`, and all typed Tauri calls in `api/tauri.ts` with contracts in `types/tauri.ts`. Rust/Tauri code is under `src-tauri/src/`; keep commands grouped by domain, including server lifecycle, REST management, configuration, backups, save migration, and modifiers. Frontend tests are in `tests/`; Rust integration tests are in `src-tauri/tests/`.

## Development Commands

Run from `Palworld/`:

```powershell
npm install
npm test                 # Vitest suite
npm run lint             # vue-tsc --noEmit
npm run build            # type-check and Vite production build
npm run tauri:dev        # desktop development app
npm run tauri:build      # NSIS release installer
```

Run `cargo fmt --all -- --check`, `cargo check --all-targets`, and `cargo test --all-targets` from `src-tauri/`. On the exFAT workspace, set `CARGO_TARGET_DIR` to an NTFS directory such as `C:\codex-target\palworld`.

## Product Architecture

Server status must reflect real PalServer processes, including instances started outside the app. Online management uses Palworld's REST API for `Info`, players, save, broadcast, and shutdown; do not restore the deprecated raw RCON client. Configuration presets are `casual`, `normal`, and `challenge` and must preserve unknown INI fields. World migration excludes source `WorldOption.sav` so server rules remain under the target server's control.

Backups use `backups/local`, `backups/server`, `backups/snapshots`, and `backups/_system`. Any save write or backup requires both PalServer and the Palworld game client to be stopped. Never modify the original fixtures under `F:\1`; test only copies.

## Style and Testing

Use Vue Composition API with `<script setup lang="ts">`, strict TypeScript, 2-space indentation, single quotes, camelCase identifiers, and PascalCase components. Views and stores must call the typed API wrapper, not `invoke()` directly. Rust commands return `Result<T, String>`, validate paths and external input, and avoid `unsafe`.

Add behavior-focused tests for success, failure, and visible state transitions. Never present fixtures, fallbacks, or stale cached values as real success. Long operations must expose their current phase immediately.

## Commits and Release

Use focused conventional subjects such as `feat:`, `fix:`, `test:`, and `docs:`. Before pushing, run frontend and Rust gates, dependency audits, `git diff --check`, and review staged files. Do not commit `.env`, runtime logs, screenshots, build output, or `F:\1` data. The application is licensed `GPL-3.0-or-later`; preserve third-party notices and source attribution.
