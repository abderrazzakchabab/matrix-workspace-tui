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
