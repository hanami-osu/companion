# Hanami Companion

Hanami Companion is a compact Tauri desktop application that connects osu! to the Hanami ecosystem. Rust owns background tracking, local process lifecycle, play detection, and native authentication; React renders the normalized state it receives.

This repository contains the first functional prototype. It can:

- discover and connect to an existing [tosu](https://github.com/tosuapp/tosu) instance over its localhost v2 WebSocket API;
- launch `tosu` automatically from `PATH` or a persisted user-selected executable, and stop it only when Companion launched that process;
- display selected beatmap and live gameplay data;
- detect meaningful passed, failed, retried, and quit attempts and retain recent activity in memory for the current session;
- authenticate with Hanami Web in the system browser using Authorization Code + PKCE;
- store refresh tokens in the operating system credential store;
- continue tracking when its native window is hidden to the system tray.

Play upload is intentionally unavailable. Hanami Web does not yet expose a production play-ingestion endpoint, and Companion never reports a submission as successful.

## Requirements

- [Bun](https://bun.sh/)
- a current stable Rust toolchain
- the [Tauri 2 system dependencies](https://v2.tauri.app/start/prerequisites/) for your platform
- [tosu](https://github.com/tosuapp/tosu/releases) available on `PATH`, selected in Settings, or launched separately
- osu! stable or osu!lazer supported by the installed tosu release

tosu is an external local service. It reads the running osu! client's state, calculates gameplay information such as PP, and publishes that state through a localhost API. Companion does not bundle, download, or silently terminate tosu.

By default, Companion probes the local tosu service during startup and launches tosu only when no existing instance responds. This preference can be disabled under **Settings → tosu → Launch with Companion**. An externally launched instance remains externally owned.

### Linux memory access

Some Linux/osu!lazer setups prevent tosu from reading game memory because of ptrace restrictions. This does not affect every Linux setup, and it is not a prerequisite for launching or connecting to tosu. Companion only marks access as **Possibly required** after a running, reachable tosu instance reports a relevant memory-read failure. A missing capability by itself remains an unknown/healthy default and never blocks stable or Wine users.

When runtime evidence exists, the advanced troubleshooting action uses the desktop's native `pkexec` authorization prompt to run `setcap` for the exact resolved native binary, verifies the capability afterward, and never runs Companion or tosu as root. It requires polkit and the libcap tools supplied by the Linux distribution. If Companion owns the running tosu process, it restarts it after access is granted; externally managed processes must be restarted by the user.

## Development

```bash
bun install
bun run tauri dev
```

Useful checks:

```bash
bun run build
bun run lint
bun run test
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
bunx tauri build
```

Release builds use the production Hanami URL, `https://hanami.yorunoken.com`. Debug builds, including `bun run tauri dev`, use `http://localhost:3000` so they can authenticate against a locally running Hanami Web server. Set `HANAMI_BASE_URL` to override either default. The active non-production URL is shown in Settings.

## Authentication and local data

Sign-in opens `/oauth/authorize` in the system browser and returns to a temporary loopback listener on `127.0.0.1`. Companion validates OAuth state, exchanges the authorization code with PKCE, keeps the access token in Rust memory, and stores only the refresh token in the native OS credential store. Tokens, authorization codes, and PKCE verifiers are never exposed to React or written to application files.

Recent plays are held in memory and disappear when Companion exits. The storage boundary is intentionally replaceable, but this prototype does not add a database.

The play detector abandons its current candidate whenever tracking or the tosu observation stream is discontinuous. An attempt becomes recordable after five seconds of active gameplay, ten judged objects, or two percent beatmap progress. Results wait for a later complete frame before being finalized, and every local attempt receives a UUID with separate start and end timestamps.

At startup, stored sessions are restored with bounded exponential retry for temporary network failures. Rejected refresh tokens are deleted. Logout always clears local credentials and in-memory access first even when remote revocation cannot be confirmed.

Rust and TypeScript snapshot models remain intentionally small and manually mirrored. Adding Specta at this stage would force the flexible lazer mod-settings payload through a broader binding layer; a Rust serialization-contract test protects the wire field names instead.

## Desktop behavior

Closing the main window hides it while tracking continues. The tray menu can reopen the window, enable or disable tracking, launch tosu, stop a Companion-owned tosu process, sign out, and quit cleanly. Disabling tracking only closes Companion's connection. An externally launched tosu process is never terminated by Companion.

The tosu socket, dashboard, and artwork endpoints are derived from one loopback configuration. Port `24050` is the current default and the persisted configuration model accepts a different port when editing it is added to the UI. A production Content Security Policy allows packaged application assets, Tauri IPC, and artwork from the local tosu HTTP endpoint; it does not permit remote scripts.
