# Project agent memory

This file is the project's committed home for project-intrinsic agent knowledge: build, test, release, architecture, and sharp-edge notes that should travel with the code.

- Add durable project-specific notes here as they are discovered through real work.

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project.
Do not repeat what the codebase already shows; point to the authoritative file or command instead.
Prefer rewriting or pruning existing entries over appending new ones.
When updating this file, preserve this bar for all agents and keep entries concise.

## Backend contract (authoritative, read-only)

The control-plane API contract lives in the separate repo `abderrazzakchabab/matrix-agent-workspace` (branch `main`): `apps/mobile/src/api/control-plane.ts`, `apps/mobile/src/api/run-events.ts`, `packages/contracts/src/{events,run,github,errors}.ts`. A local clone exists at `/home/isc-cha/private/firstmate/projects/matrix-agent-workspace`. The full implementation plan for this repo (task-by-task, TDD) is `docs/superpowers/plans/2026-08-15-matrix-workspace-tui.md`.

## Plan corrections (Group 3, api-client resources)

Three places where the plan's literal code does not compile or does not match the wire contract; all were corrected during implementation, so the code in `crates/api-client` is authoritative and the plan text is stale:

- `ControlPlaneApi::set_cookie` takes `&mut self` (per plan Task 2.2, already committed). The Group 3 test snippets (Tasks 3.2–3.12) omit `mut` on `let client` before calling `set_cookie`; the tests need `let mut client`.
- `GithubWriteScope` must serialize to `issues:write` / `pull_requests:write` (colons, per `control-plane.ts`). The plan's `#[serde(rename_all = "snake_case")]` alone would emit `issues_write`; the enum carries per-variant `#[serde(rename)]` to keep the colon values.
- Plan Task 3.7's `api/github.rs` imports `GithubPullRequestSummary` before the type is added (Task 3.8). Keep imports aligned with the types that exist at each stage; final state matches the plan.

## Plan corrections (Group 4, api-client SSE)

Four places where the Group 4 plan's literal code does not compile or contradicts its own contract; corrected during implementation, so `crates/api-client` is authoritative and the plan text is stale:

- Task 4.1 test `handles_windows_line_endings_and_missing_field_value`: `parse_sse_frame("id: 1\n\n")` returns `None` (no `data:` field → ignored, per both the plan's impl and the reference `parseSseFrame`); the test's `.unwrap()` panicked. Fixed the input to `"id: 1\ndata:\n\n"` (empty `data:` value), assertions unchanged.
- Task 4.5 `stream_skips_malformed_frames_without_stopping`: the `garbage` body concatenation starts with a `&str` literal, so `&str + &str` fails to compile. Fixed with `": heartbeat\n\n".to_string()` on the first operand.
- Task 4.7 `sse_surface_is_public` (lib.rs): `TERMINAL_EVENT_TYPES` is a `const`, not a type, so `Option<crate::sse::TERMINAL_EVENT_TYPES>` does not compile (changed to `let _ = crate::sse::TERMINAL_EVENT_TYPES;`); and `parse_sse_frame`'s fn pointer type needs the `&str` parameter: `fn(&str) -> Option<SseFrame>`.
- Structural: plan steps say "append to the tests module", but a bare `cat >>` lands test code after the module's closing brace (compiles as module-level items, so test counts still pass while httpmock prelude scope breaks). Insert tests before `mod tests`' closing brace and keep `use httpmock::prelude::*;` inside the module.

Verified against the authoritative contract (repo `abderrazzakchabab/matrix-agent-workspace` @ commit `063e2e1`): `RunEvent` fields, the 18 `RUN_EVENT_TYPES`, `TERMINAL_TYPES` (`run.completed`, `run.partial`, `run.failed`, `run.cancelled`), digit-only SSE id ↔ strict integer sequence, runId match, and resume-from-`highestSequence` all mirror `packages/contracts/src/events.ts` + `apps/mobile/src/api/run-events.ts`. Note the plan's sse.rs code blocks are not all rustfmt-clean at 100 cols (long lines); the plan text claims they are.

## Group 5 (state, SessionStore): plan-literal, no corrections

Group 5 (Tasks 5.1–5.4) compiled and passed as written — no plan corrections unlike Groups 3/4. `crates/state/src/session_store.rs` is authoritative. Same caveat as Group 4 applies: the plan's code blocks are not rustfmt-clean at 100 cols (e.g. `load`'s `map_err` chain, the 0600 mode assertion, `default_path` test line); the no-mistakes pipeline applies `cargo fmt` in its fix round. Run cargo through the rustup 1.85.0 toolchain (`rustup which cargo` / prepend its bin dir) — the Homebrew `cargo` 1.93.1 shadows the rustup shim on this machine.

## Group 6 (state, screens): plan-literal, two stale spots

Group 6 (Tasks 6.1–6.8) implemented the plan's code as written — `crates/state/src/screens.rs` is authoritative. Two places where the plan text is stale:

- Task 6.5's intermediate tree cannot compile: `GitHubWorkspaceState::begin_mutation` calls `command_hash` from lib code (not just the draft test), so the whole crate fails `cargo test -p state` at 6.5 with `cannot find function command_hash`. The plan claims the three non-hash 6.5 tests pass there — impossible; the 6.5 commit lands red by design and the suite only compiles once Task 6.6 adds the helpers. Follow the plan's commit sequence anyway.
- The plan's per-task test counts are miscounted (claims 12/16/19/21/24/25; actual is 12/16/20/22/29/30 — 6.3 and 6.5–6.7 are each off by one). The Task 6.8 suite gate actually reports api-client 57 tests + state 30 tests. Same rustfmt caveat as Groups 4/5: the plan's screens.rs blocks are not rustfmt-clean; the no-mistakes pipeline applies `cargo fmt` in its fix round.

## Group 7 (tui, app shell): five corrections to plan-literal code

Group 7 (Tasks 7.1–7.6) — `crates/tui/src/app.rs` + `main.rs` + stub screens — is authoritative; the plan text is stale in five places, all corrected during implementation:

- **ratatui 0.29 `Frame` is not generic**: the plan's render stubs and `App::draw` use `Frame<B>` / `fn render_x<B: Backend>(.., &mut Frame<B>, ..)`, but 0.29's `Frame<'a>` is lifetime-only and `Terminal::draw` takes `FnOnce(&mut Frame)`. All render fns and `draw` take `&mut Frame` with no `Backend` param; the `Backend` import is dropped from the stubs. Group 8 must keep this signature.
- **Task 7.1 references `abort_stream` before it exists** (defined in Task 7.4): 7.1 committed a no-op `fn abort_stream(&mut self) {}` stub; Task 7.4 replaced it with the real implementation (abort task + drop rx).
- **Task 7.4 does not compile as written**: (a) `execute_command`'s full match references the Task 7.5 methods before they exist — keep the 7.5 arms commented with a `_ => {}` catch-all until 7.5 (same comment-until-implemented mechanism the plan used for 7.3→7.4); (b) `enter_run`'s spawn closure captures `run_id` by move, then `RunState::new(run_id, ..)` uses it after the move — clone into `stream_run_id` for the closure; (c) `drain_stream_events`'s `let Some(rx) = &mut self.stream_rx` conflicts with `self.current_mut()`/`self.expire_session()` inside the loop — drain into a local `Vec<AppEvent>` first, then apply; (d) the `launch_run` SSE test concatenates two `&str` literals with `+` — first operand needs `.to_string()` (same fix as Group 4 Task 4.5).
- Test counts are off again: the Task 7.6 gate reports api-client 57 + state 30 + tui 18 (plan claims 52/23/14). Same rustfmt caveat: the plan's app.rs blocks are not fmt-clean; `cargo fmt` was applied at Task 7.6. The `Command` variants and render stubs are only constructed/called from Group 8, so unused-import/variant warnings are expected on the 7.1–7.6 commits.
- **Task 7.5's `confirm_mutation` enqueue error path omits session expiry**: the plan maps the enqueue error straight to `MutationFlowStatus`, but the design spec's "401 anywhere clears the session" contract requires expiry handling there too. The review commit (`678498e`) added `error.is_session_expired() → self.expire_session(); return;` before mapping APPROVAL_EXPIRED/denial/failed. The approval-error arm already routes through `github_error`, which handles expiry.

## Plan corrections (Group 8, tui screens)

Group 8 (Tasks 8.1–8.13) implemented the plan's handlers/renders as written — `crates/tui/src/screens/*.rs` is authoritative; the plan text is stale in several places:

- **ratatui 0.29.0 `Layout::split` is nondeterministic when constraints oversubscribe the area** (fixed in no 0.29.x stable — only 0.29.0 exists). `[3, Min(4), 3, 2, 1]` on a 12-row area flips between `[3,4,2,2,1]` (create-field content dropped) and `[3,4,3,1,1]` across processes, so the 8.4 render test flaked. Rule that now holds for every Group 8 render: constraints must sum to ≤ the test's terminal height. Corrections: workspaces `Min(4)`→`Min(3)` (12-row test), run `Min(6)`→`Min(5)` (18-row test), github `Min(6)`→`Min(5)` + confirmation row `Length(2)`→`Length(6)` (18-row test; a 2-row area can never show the multi-line confirmation paragraph, so the 8.12 test fails with plan-literal constraints). Exact-fit layouts (`ws_min3`, `login`) probe deterministic; keep the code comments explaining the Min deviation.
- **The plan's 8.3 test line `assert_eq!(state.selected(), 0)` does not compile**: `WorkspacesState::selected()` returns `Option<&WorkspaceSelection>` (state crate, Group 6, is authoritative). The test asserts the public field directly: `assert_eq!(state.selected, 0)`.
- **ratatui 0.29 `Frame` is not generic**: every Group 8 render signature is `fn render_x(state, frame: &mut Frame, area: Rect)` with no `Backend` param/import, even though the plan prints `<B: Backend>` / `Frame<B>` (same correction as Group 7).
- **The plan's Task 8.1 full file drops `render_login`**, but `App::draw` calls it until Task 8.2 replaces it — the 8.1 commit keeps the stub `render_login` (and 8.3/8.5/8.7/8.9/8.11 likewise keep their render stubs until the next task).
- Test counts again: the Task 8.13 gate reports api-client 57 + state 30 + tui 40 (plan claims 52/25/40; only the tui 40 is right); the post-gate review commits (9f0a17a, 9895015, a3ba29c) added three screen tests, so the final suite is tui 43 (a3ba29c: 43/43 green). Same rustfmt caveat as Groups 4–7: the plan's screen blocks are not fmt-clean at 100 cols; `cargo fmt` was applied at Task 8.13. The `handle_room_binding_key`/`handle_run_key` `state` params are intentionally unused (plan-literal) and are named `_state` to keep the plan's signature without warnings.
- **Post-gate review commits changed plan-literal keybinding behavior** (captain decision on review findings F1/F2/F3, commit `9f0a17a`; plus `9895015`, `a3ba29c` — deliberate, keep it): while a text field is active (prompt in RunComposer, mutation title in GitHubWorkspace, name in Workspaces create mode) keystrokes go to the field and command keys are not intercepted; command keys operate only when the field is empty. Esc cancels the workspaces create mode and the github mutation mode (clearing the draft title); the github mutation-mode hint reads `(Enter: review, Esc: cancel)`, not the plan's `q: cancel`. The mutation control (`m`) is gated on a non-revoked write grant — without one it sets an error and the hint line omits `m`; Enter in mutation mode requires a selected repository. Login's `q` quits only while both fields are empty (URLs/tokens contain `q`; the plan's `q_quits` test was unconditional). Each site is marked with a code comment.

## Plan corrections (Group 9, npm launcher): four places the plan text is wrong or incomplete

Group 9 (Tasks 9.1–9.2) — `npm/matrix-workspace-tui/` (package.json, index.js, scripts/platform.js, scripts/download.js, test/*.test.js) is authoritative; the plan text is stale or un-runnable in four places, all corrected during implementation:

- **The plan's download tests deadlock as written**: `runDownload` used `spawnSync`, which blocks the test process's event loop — but the static HTTP server lives in that same process, so it can never answer the child's requests and the child hangs forever (this wedged the run for ~75 min). Fix: `runDownload` now uses async `spawn` + a Promise resolving `{status, stdout, stderr}` (event loop free to serve HTTP while the child runs), and the idempotent test `await`s the first `runDownload` (otherwise both runs race and the `hits.binary === 1` assertion flakes).
- **`fetchBinary` had no timeout** (supervisor-required fix): `postinstall`/download against the real GitHub URL — which has no release until Group 10 publishes one — could hang forever. `fetchBinary` now does `request.setTimeout(MATRIX_WORKSPACE_TUI_DOWNLOAD_TIMEOUT_MS || 30000)` + `request.destroy(err)` so the error path rejects cleanly, and `main`'s catch `rmSync`s `BIN_DIR` so no partial tmp/checksum/dir is left behind (this also makes the plan's "no partial binary left behind" assertion actually pass — with plan-literal code, `bin/` survives a checksum-mismatch failure because the `.sha256` file was written there). A fourth download test covers the timeout path (server holds the connection open, 500ms timeout, asserts non-zero exit + `/timed out/` + no `bin/`). The `bin/` dir is now in `.gitignore`.
- **`package.json`'s `"test": "node --test test/"` is broken on Node 26** (the only Node on this machine): newer test runners treat the directory arg as an entry module → `MODULE_NOT_FOUND`, zero tests run. Changed to `"test": "node --test"` (bare; auto-discovers `test/*.test.js`, works Node ≥ 18). Acceptance criterion is bare `node --test`, which passes: launcher 2 + download 4 = 6 tests green.
- Test count: plan says `# pass 5` (2 launcher + 3 download); actual is 6 (the added timeout test). The download tests need `server.close()` to destroy tracked sockets first (`closeAllConnections`-style), otherwise `close()` waits on the never-answered connection.

## Group 10 (CI/release): workflows committed, plan's 10.3 counts stale

Group 10 (Tasks 10.1–10.3) landed `.github/workflows/ci.yml` + `release.yml` plan-literal — the workflow YAML is authoritative. Facts that differ from the plan text:

- **The plan's Task 10.3 test counts are stale** (same pattern as Groups 4–8): it claims api-client 52 / state 25 / tui 40 and npm `# pass 5`; actual is api-client 57 / state 30 / tui 43 and npm pass 6. The 10.3 gate checks that everything passes, not the counts.
- **`.no-mistakes.yaml` had its `no_ci: true` declaration removed** in the `chore: final verification` commit — CI is now real (ci.yml runs on PRs + pushes to main, so PR checks register pre-merge; release.yml is tag-only). The file now carries only a comment.
- **`release.yml` publishes npm on every `v*` tag and needs an `NPM_TOKEN` secret** (Settings → Secrets and variables → Actions) with publish rights to the `matrix-workspace-tui` scope; the tag must match `package.json` `version` because `postinstall` downloads `.../releases/download/v<version>/...`. Documented as a comment in release.yml; ci.yml never depends on it.
- **Post-gate review commit `f91f6ff` deviates from the plan's release.yml in two places**: (a) the checksum step falls back to `shasum -a 256` when `sha256sum` is missing — macOS runners don't ship `sha256sum`, so the plan's bare call would fail both macos matrix jobs; (b) the `release` job declares `permissions: contents: write` because softprops/action-gh-release@v2 needs it to create the release (GITHUB_TOKEN's default is read-only). The workflow YAML is authoritative.
- `dtolnay/rust-toolchain@stable` with no `toolchain` input uses the pinned `rust-toolchain.toml` (1.85.0). If `ubuntu-24.04-arm`/`macos-14` runner labels are unavailable in a fork, drop that matrix row — the npm launcher still covers all four platform names.
