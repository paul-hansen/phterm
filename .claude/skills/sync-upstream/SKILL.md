---
name: sync-upstream
description: Fetch the latest wezterm/wezterm and rebase this fork's patches on top of it. Use when asked to sync, update from, or rebase onto upstream wezterm.
---

# Sync with upstream wezterm

This repo (`paul-hansen/phterm`) is a fork of `wezterm/wezterm` that carries a
small stack of patches on `main`. Syncing means rebasing that stack onto
`upstream/main`.

## Steps

1. **Check the starting state.** The working tree must be clean and `main`
   checked out. If not, stop and tell the user.

2. **Fetch upstream without tags.**
   ```console
   $ git remote get-url upstream || git remote add upstream https://github.com/wezterm/wezterm.git
   $ git config remote.upstream.tagOpt --no-tags
   $ git fetch --no-tags upstream
   ```
   Never fetch upstream tags: the fork intentionally only has its own release
   tags, and pushing many tags at once suppresses the release workflow.

3. **Show what will be rebased.** List the fork's patches
   (`git log --oneline upstream/main..main`) and how many upstream commits are
   new (`git rev-list --count main..upstream/main`). If there are none, say so
   and stop.

4. **Rebase.** Save a backup ref first so the user can always get back:
   ```console
   $ git branch -f backup/pre-sync main
   $ git rebase upstream/main
   ```
   Resolve conflicts as follows:
   - **Workflows under `.github/workflows/` other than `release.yml`, and
     `ci/generate-workflows.py`**: the fork deleted these on purpose. Keep them
     deleted (`git rm`), even if upstream modified them. Upstream may also add
     new workflows; delete those too and mention them to the user.
   - **`CLAUDE.md`, `README.md`, `.github/workflows/release.yml`**: keep the
     fork's version, but fold in upstream changes that still apply (e.g. new
     build commands in CLAUDE.md).
   - **Anything else (real code, e.g. `term/`, `wezterm-escape-parser/`)**:
     do not guess. Show the user the conflict, explain both sides, and propose
     a resolution before continuing.

   If a patch has been merged upstream, `git rebase` drops it as empty; report
   which ones were dropped.

5. **Verify.**
   ```console
   $ make check
   $ cargo nextest run -p term
   $ cargo nextest run -p wezterm-escape-parser
   ```
   Report failures with their output; do not push a broken sync.

6. **Ask before pushing.** Summarize: upstream commits pulled in, patches
   kept/dropped, conflicts resolved and how. Then ask the user whether to run
   `git push --force-with-lease origin main`. Never force-push without an
   explicit yes.

7. **Offer a release.** After pushing, offer to cut a release:
   ```console
   $ ci/tag.sh
   $ git push origin <the new tag>
   ```
   Push only that one tag, never `git push --tags`. Then monitor the `release`
   workflow (`gh run list -R paul-hansen/phterm`).

Once everything is pushed and working, delete the backup:
`git branch -D backup/pre-sync`.
