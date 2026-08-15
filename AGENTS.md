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
