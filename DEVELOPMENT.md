# Developing FlipOn

FlipOn is a Tauri 2 app: a Rust core (accounts, switching, usage polling, the tray
icon) and one React + TypeScript front end that renders two windows, the popover and
Settings. UI components follow [shadcn/ui](https://ui.shadcn.com) on Radix UI, with
Font Awesome icons.

## Commands

Needs Node 22+, pnpm, Rust (stable), and on Windows the Visual Studio C++ build tools
with WebView2.

| What | Command |
| --- | --- |
| Install | `pnpm install` |
| Run in development | `pnpm tauri dev` (front end on port 1520) |
| Rust tests | `cd src-tauri && cargo test` |
| Rust lint | `cd src-tauri && cargo clippy --all-targets` |
| Type check the front end | `pnpm exec tsc --noEmit` |
| Release installer | `pnpm tauri build` |

UI work without Rust: `pnpm dev`, then open `http://localhost:1520/?w=popover` or
`?w=settings`. Outside Tauri a mock backend (`src/lib/mock.ts`) supplies the data.

Regenerate every icon from `app-icon.png` (1024 px) with `pnpm tauri icon app-icon.png`.
The logo source is `docs/logo.svg`. To see every tray look as PNGs (dark and light taskbar):
`FLIPON_DUMP_DIR=some/folder cargo test dump_tray_icons`.

## Where things are

| File | What it does |
| --- | --- |
| `src-tauri/src/engine.rs` | The brain: account list, which account is active, usage polling, switching, the login flow, notifications. Everything the UI shows is a `Snapshot` built here. |
| `src-tauri/src/strategy.rs` | The four switching strategies as pure functions, with unit tests. |
| `src-tauri/src/claude_creds.rs` | Reads and writes the CLI's own login files. Replaces only `claudeAiOauth` and `oauthAccount`. |
| `src-tauri/src/store.rs` | FlipOn's own files: settings, state and one saved login per account. |
| `src-tauri/src/api.rs` | The usage endpoint and OAuth token refresh. Parses both response shapes. |
| `src-tauri/src/cli.rs` | Finds `claude` and runs `claude auth login`, forwarding the pasted code. |
| `src-tauri/src/activity.rs` | "Today" cost, turns and lines, from the local session transcripts. |
| `src-tauri/src/tray.rs`, `tray_icon.rs` | The tray menu, and the icon drawn with tiny-skia. |
| `src-tauri/src/windows.rs` | The popover (anchored to the tray icon) and the Settings window. |
| `src-tauri/src/commands.rs` | Everything the front end can call. |
| `src/components/popover/` | The popover: account cards, notices, dialogs. |
| `src/components/settings/` | The Settings window. |
| `src/components/ui/` | The shadcn style base components. |

FlipOn's data lives in `%APPDATA%\FlipOn\`: `settings.json`, `state.json` and
`credentials\<id>.json`.

## Things to know

* Windows: creating a window inside a synchronous command or a menu handler
  deadlocks the main thread. Keep `open_settings` async.
* The popover is a transparent, frameless window. It hides itself when it loses focus,
  and a tray click within 350 ms of that is ignored so it does not reopen at once.
* The usage endpoint is undocumented. `api::parse_usage` accepts the flat keys
  (`five_hour`, `seven_day`) and the newer `limits` array.
* Tests never touch your real login: the one that writes credentials points
  `CLAUDE_CONFIG_DIR` at a temporary folder.

## Debugging the real window

Set `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` before
`pnpm tauri dev`, then attach to `http://127.0.0.1:9222/json` with any DevTools client.
Do not leave that port open.

## Releasing

Not automated yet. `pnpm tauri build` produces the Windows installer in
`src-tauri/target/release/bundle/`. Unsigned installers trigger a SmartScreen warning.
