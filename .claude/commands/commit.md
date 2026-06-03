---
description: Commit working-tree changes as canonical conventional commits, after side-specific checks
argument-hint: (optional) what to emphasize or how to split the commits
allowed-tools: Bash(git status:*), Bash(git diff:*), Bash(git add:*), Bash(git commit:*), Bash(git rev-parse:*), Bash(pnpm check:fix:*), Bash(pnpm verify:*), Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(cargo test:*)
---

Commit the current working-tree changes (staged, unstaged, and untracked) as canonical
conventional commits, following `CLAUDE.md` §3. Optional guidance from the user:

$ARGUMENTS

Follow these steps:

1. **Guard.** Confirm you are on a feature branch in a worktree, not on `main` in the main working
   copy (`git rev-parse --abbrev-ref HEAD`). If on `main`, stop and tell the user to start work
   with `/new-pr` first — never commit on `main`.

2. **Inspect changes.** Run `git status` and `git diff` (include untracked files). Determine which
   side(s) changed:
   - **Frontend** — files under `src/`, or root TS/build config.
   - **Backend** — files under `src-tauri/`.

3. **Auto-fix, then verify the touched side(s).** Stop and report if anything fails — do **not**
   commit on a red check.
   - Frontend: `pnpm check:fix` then `pnpm verify`
   - Backend (run from `src-tauri/`): `cargo fmt --all`, then
     `cargo clippy --all-targets -- -D warnings`, then `cargo test --locked`
   - Both touched: run both sets.

4. **Re-inspect.** Auto-fix may have modified files — re-run `git status`/`git diff` so commits
   reflect the final state.

5. **Group into logical commits.** Split the changes into multiple conventional commits when they
   span distinct concerns; use a single commit when they form one coherent change. For each group:
   `git add <specific paths>` then commit. Honor any splitting/emphasis guidance the user gave.

6. **Message format.** Conventional commits: `<type>(scope): <imperative summary>` (scope optional),
   with a body when it adds context. Types: `feat`, `fix`, `chore`, `docs`, `refactor`, `test`,
   `perf`. Keep the subject concise (~72 chars). End every commit message with the trailer:
   ```
   Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
   ```

7. **Report** the commits created (hashes + subjects).
