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

## Group 7 (tui, app shell): four corrections to plan-literal code

Group 7 (Tasks 7.1–7.6) — `crates/tui/src/app.rs` + `main.rs` + stub screens — is authoritative; the plan text is stale in four places, all corrected during implementation:

- **ratatui 0.29 `Frame` is not generic**: the plan's render stubs and `App::draw` use `Frame<B>` / `fn render_x<B: Backend>(.., &mut Frame<B>, ..)`, but 0.29's `Frame<'a>` is lifetime-only and `Terminal::draw` takes `FnOnce(&mut Frame)`. All render fns and `draw` take `&mut Frame` with no `Backend` param; the `Backend` import is dropped from the stubs. Group 8 must keep this signature.
- **Task 7.1 references `abort_stream` before it exists** (defined in Task 7.4): 7.1 committed a no-op `fn abort_stream(&mut self) {}` stub; Task 7.4 replaced it with the real implementation (abort task + drop rx).
- **Task 7.4 does not compile as written**: (a) `execute_command`'s full match references the Task 7.5 methods before they exist — keep the 7.5 arms commented with a `_ => {}` catch-all until 7.5 (same comment-until-implemented mechanism the plan used for 7.3→7.4); (b) `enter_run`'s spawn closure captures `run_id` by move, then `RunState::new(run_id, ..)` uses it after the move — clone into `stream_run_id` for the closure; (c) `drain_stream_events`'s `let Some(rx) = &mut self.stream_rx` conflicts with `self.current_mut()`/`self.expire_session()` inside the loop — drain into a local `Vec<AppEvent>` first, then apply; (d) the `launch_run` SSE test concatenates two `&str` literals with `+` — first operand needs `.to_string()` (same fix as Group 4 Task 4.5).
- Test counts are off again: the Task 7.6 gate reports api-client 57 + state 30 + tui 18 (plan claims 52/23/14). Same rustfmt caveat: the plan's app.rs blocks are not fmt-clean; `cargo fmt` was applied at Task 7.6. The `Command` variants and render stubs are only constructed/called from Group 8, so unused-import/variant warnings are expected on the 7.1–7.6 commits.
