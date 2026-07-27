# Repository Guidelines

## Project Structure & Module Organization

`Palworld/` contains the production desktop application. Its Vue 3 UI lives in `Palworld/src/`: views belong in `views/`, reusable UI in `components/`, application state in `stores/`, and typed Tauri calls in `api/tauri.ts` with matching definitions in `types/tauri.ts`. The Rust/Tauri backend is under `Palworld/src-tauri/src/`; keep commands grouped by domain such as `server.rs`, `config.rs`, and `rcon.rs`.

Frontend tests are in `Palworld/tests/`. Product, architecture, research, release, and historical documents are under `docs/`; begin with `docs/README.md`. The production application is the sole deployable codebase in this repository.

## Build, Test, and Development Commands

Run commands from `Palworld/`:

```powershell
npm install             # install frontend dependencies
npm run dev             # start the Vite UI server
npm run lint            # run vue-tsc without emitting files
npm run build           # type-check and create dist/
npm run verify          # lint, then production build
npx vitest run          # run the jsdom Vue test suite
npm run tauri:dev       # run the desktop app in development
npm run tauri:build     # create the release desktop package
```

For backend-only validation, run `cargo check` from `Palworld/src-tauri/`.

## Coding Style & Naming Conventions

Use TypeScript strict mode and Vue Composition API with `<script setup lang="ts">`. Use 2-space indentation, single quotes, camelCase for functions and variables, and PascalCase for Vue components (for example, `ServerStatusCard.vue`). Keep user-facing UI text and Rust command errors in Chinese. Put shared visual tokens and global styles in `Palworld/src/style.css`.

Components must call typed functions in `src/api/tauri.ts`; do not invoke Tauri commands directly from views or stores. Rust commands should return `Result<T, String>`, validate external input, and avoid `unsafe`.

## Testing Guidelines

Vitest uses jsdom and discovers `tests/**/*.spec.ts`. Name tests by behavior, such as `server-store.spec.ts`, and cover success, failure, and state-transition paths for changed functionality. Run `npx vitest run` and `npm run build` before submitting UI or API changes.

## Commit & Pull Request Guidelines

The existing history uses concise conventional-style subjects, e.g. `feat: initial commit — Palworld Server Manager (Tauri2 + Vue3)`. Use `feat:`, `fix:`, `docs:`, `refactor:`, or `test:` followed by an imperative summary. Keep commits focused.

Pull requests should explain the user-visible change, list validation commands run, link relevant issues or documents, and include screenshots for visual changes. Do not commit `.env`, runtime logs, generated screenshots, or installed dependencies.

## Configuration & Security

Copy `Palworld/.env.example` to `Palworld/.env` for local configuration. Never hardcode credentials, and shell-escape any user-controlled values passed to PowerShell.
