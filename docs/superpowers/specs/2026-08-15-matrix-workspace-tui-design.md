# Design — matrix-workspace-tui

A desktop TUI client for the Matrix Agent Workspace control plane, mirroring the feature set of the existing Expo mobile app. Written in Rust with ratatui, installed via npx (a thin npm launcher that downloads a prebuilt platform binary from GitHub releases).

## 1. Architecture & components

A small Cargo workspace with three crates:

```
matrix-workspace-tui/
├── Cargo.toml                 # workspace root
├── crates/
│   ├── api-client/            # typed HTTP + SSE against the control plane
│   ├── state/                 # session store, app state machine, run/approval state
│   └── tui/                   # ratatui screens (the binary)
├── npm/
│   └── matrix-workspace-tui/  # thin launcher package (postinstall download)
└── .github/workflows/         # CI: test + build per platform + publish
```

- **api-client**: reqwest + tokio; every control-plane route the mobile app uses, with the same typed shapes (workspace, room, run, event, GitHub read pages, grant, approval, mutation, audit). The SSE replay stream (`GET /api/runs/:runId/events`) becomes an async `EventStream` that yields validated `RunEvent`s and resumes from the last known `sequence`.
- **state**: `SessionStore` (control-plane session cookie in `~/.config/matrix-workspace-tui/`, mode 0600), the screen state machine (Login → Workspaces → Rooms → RunComposer/Run → GitHubWorkspace), and per-screen state structs. Pure logic, no I/O, unit-testable.
- **tui**: ratatui screens mirroring the mobile navigation: Login, Workspaces, RoomBinding, RunComposer, Run (live timeline + terminal result), GitHubWorkspace (read panels + mutation confirmation + audit). One keybinding model (hjkl/arrows, Enter, q).

## 2. Data flow

1. **Login**: paste control-plane session token → `POST /api/auth/matrix/session` → capture the session cookie → store locally (mode 0600). On 401 anywhere, clear the session and return to Login.
2. **Workspaces**: list/create; selection carries `workspaceId` onward.
3. **Rooms**: list + bind room↔workspace.
4. **Run**: composer submits prompt/mode/specialists/roomId with a fresh idempotency key; the Run screen opens the SSE stream, renders events as they arrive (specialist progress, terminal states), supports cancel, and shows authoritative Matrix-delivery status from the run endpoint — never inferred from the stream.
5. **GitHub workspace**: read panels (repos/issues/PRs) → request write grant → explicit confirmation showing scope/repo/args → enqueue mutation (idempotency key) → audit history view.

## 3. Error handling

- API failures → inline status bar messages with the backend's error code, never a crash.
- SSE drop/reconnect → resume from last `sequence` with a "reconnecting" indicator; stale terminal events ignored once a valid terminal event is stored (same policy as mobile).
- Malformed wire events → ignored, stream continues until a valid terminal event (matches the Phase B contract).
- Config dir unreadable/session expired → clean message + return to Login.

## 4. Testing & delivery

- Unit tests in each crate (api-client against a mock HTTP server, state machine transitions, event validation/replay).
- npm launcher: `postinstall` downloads the matching platform binary from GitHub releases (linux-x64/aarch64, darwin-x64/aarch64) with a checksum check; the `bin` entry runs it. CI builds all four, attaches to a release, publishes the npm package with the private repo as the source.
