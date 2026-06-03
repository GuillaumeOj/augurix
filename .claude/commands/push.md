---
description: Push the current branch and open a PR (or update the existing one); force-with-lease on divergence
argument-hint: (none)
allowed-tools: Bash(git push:*), Bash(git rev-parse:*), Bash(git status:*), Bash(git log:*), Bash(git ls-remote:*), Bash(gh pr:*)
---

Push the current branch to `origin` and ensure a PR exists, following `CLAUDE.md` §4.

Follow these steps:

1. **Guard.** Determine the current branch (`git rev-parse --abbrev-ref HEAD`). If it is `main`,
   stop. Confirm there are commits to push; if the working tree has uncommitted changes, tell the
   user to run `/commit` first.

2. **Detect remote state.** Check whether the branch already exists on the remote:
   `git ls-remote --heads origin <branch>`. If it exists, compare local vs remote to decide if
   history is a fast-forward or has **diverged** (e.g. after `git commit --amend` or a rebase).

3. **Push** accordingly:
   - No remote branch yet → `git push -u origin <branch>`
   - Remote exists, fast-forward → `git push`
   - Remote exists but diverged → `git push --force-with-lease`

4. **Ensure a PR exists.** Check with `gh pr view` / `gh pr list --head <branch>`.
   - **No PR** → create one:
     ```bash
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
     Derive the title (`<type>: <summary>`) and body from the commits on the branch
     (`git log origin/main..HEAD`). Keep the three sections (`## Summary`, `## Highlights`,
     `## Test plan`) exactly.
   - **PR exists** → the push already updated it; do not create a duplicate.

5. **Report** the PR URL.
