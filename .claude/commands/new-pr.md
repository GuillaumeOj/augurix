---
description: Start a new task in a fresh worktree following Augurix conventions
argument-hint: <description of the work, optionally prefixed with a type>
allowed-tools: Bash(git fetch:*), Bash(git worktree:*), Bash(git branch:*), Bash(git status:*), AskUserQuestion
---

You are starting a brand-new task. Set up a worktree following the Augurix conventions in
`CLAUDE.md` §1–2. The task description is:

$ARGUMENTS

Follow these steps:

1. **Read the description.** If it begins with a known type word
   (`feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `perf`), treat that as the inferred type
   and drop it from the description. Otherwise, infer the most fitting type from the intent
   (new capability → `feat`, bug → `fix`, tooling/chores → `chore`, etc.).

2. **Confirm the type with the user** via `AskUserQuestion` — always ask, even when you inferred
   one confidently. Offer the inferred type first (labelled "(Recommended)") followed by the other
   types from the list above.

3. **Derive a kebab-case slug** from the description: lowercase, hyphen-separated, concise
   (~2–4 words), no leading type word. Example: "fix the diff scroll glitch" → `diff-scroll`.

4. **Compute the names** per §2:
   - Worktree dir: `<type>-<slug>` (placed at `.claude/worktrees/<type>-<slug>`)
   - Branch: `<type>/<slug>` — the worktree name with the **first** `-` replaced by `/`.

5. **Create the worktree** based on the latest `main`:
   ```bash
   git fetch origin
   git worktree add .claude/worktrees/<type>-<slug> -b <type>/<slug> origin/main
   ```

6. **Guard against collisions.** Before creating, check `git worktree list` and
   `git branch --list <type>/<slug>`. If the directory or branch already exists, stop and ask the
   user for a different description instead of overwriting anything.

7. **Report** the new worktree path and branch name, and remind the user that subsequent work for
   this task — including `/commit` and `/push` — happens inside that worktree.
