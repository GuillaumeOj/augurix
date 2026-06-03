# Augurix — Working Conventions

This file documents the workflow Claude (and humans) follow when working on this repo.
Augurix is a Tauri 2 desktop app: React + TypeScript frontend in `src/`, Rust backend in `src-tauri/`.

## 1. Always work in a worktree

Never commit on `main` in the main working copy. For any new task, create a worktree first.

## 2. Naming convention

- **Worktree directory:** `<type>-<kebab-description>` (e.g. `feat-add-button`), placed at
  `.claude/worktrees/<type>-<kebab-description>`.
- **Branch (local & remote):** `<type>/<kebab-description>` (e.g. `feat/add-button`).
- **Rule:** the branch is the worktree name with the **first** `-` replaced by `/`
  (e.g. worktree `fix-diff-scroll` → branch `fix/diff-scroll`).
- **Types:** `feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `perf`.

Create a worktree:

```bash
git worktree add .claude/worktrees/feat-add-button -b feat/add-button
```

> Note: some existing worktrees use a `worktree-<name>` branch scheme — that is the Augurix
> product's own internal convention. New development work uses the `<type>/...` scheme above.

## 3. Before committing — verify the touched side(s)

Check which side changed (`git status` / `git diff --name-only`) and run only the matching checks.
Auto-fix formatting first, then confirm. These mirror CI (`.github/workflows/ci.yml`), so a green
local run means green CI.

- **Frontend touched** (files under `src/`, or root TS/build config):

  ```bash
  pnpm check:fix     # auto-fix biome lint/format (run if needed)
  pnpm verify        # typecheck (tsr generate && tsc --noEmit) + biome check + vitest run
  ```

- **Backend touched** (files under `src-tauri/`) — run from `src-tauri/`:

  ```bash
  cargo fmt --all                            # auto-format
  cargo clippy --all-targets -- -D warnings
  cargo test --locked
  ```

- **Both touched:** run both sets.

Only commit once the relevant checks pass.

## 4. On push — always open a PR

```bash
git push -u origin feat/add-button
gh pr create --base main --assignee @me \
  --title "<type>: <summary>" \
  --body "$(cat <<'EOF'
## Summary

<what changed and why, in a sentence or two>

## Highlights

- <notable change or decision>
- <another>

## Test plan

- <how it was verified — commands run, checks passed, manual steps>
EOF
)"
```

PRs are opened ready-for-review against `main`. Every PR:

- is **assigned to the commit author** (`--assignee @me` — the author opening the PR);
- has a **body following this fixed template**: `## Summary`, `## Highlights`, `## Test plan`.
