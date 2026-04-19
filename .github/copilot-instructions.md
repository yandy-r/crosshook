# GitHub Copilot / code-agent PR guidance

This file is read by GitHub Copilot's coding agent. OpenAI Codex and the
Anthropic code-agent bots read [`AGENTS.md`](../AGENTS.md) /
[`CLAUDE.md`](../CLAUDE.md), which are the canonical agent rules — the points
below are the PR-workflow essentials re-stated for assistants that land here
first.

## PR title — Conventional Commits, enforced

PR titles are validated by
[`.github/workflows/pr-title.yml`](workflows/pr-title.yml) and the check is
required to merge. Use:

```
<type>[optional scope]: <description>
```

- Allowed types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`,
  `ci`, `chore`, `style`. Version-bump semantics: see
  [`CLAUDE.md`](../CLAUDE.md) § _Git & Conventional Commits_.
- **Never open a PR with `[WIP]`, `Draft:`, `Initial plan`, or an untagged
  title.** The workflow rejects these prefixes. For work-in-progress, use
  GitHub's native **Draft PR** status instead.
- The PR title becomes the **squash-merge commit subject verbatim** and
  appears in `CHANGELOG.md`. Write it exactly as it should read in `git log`.
- Internal-only docs (files under `docs/plans/`, `docs/research/`,
  `docs/internal/`) must use `docs(internal): …`.
- Fix a rejected title with `gh pr edit <n> --title "<new>"` (or the GitHub
  UI). Do **not** open a replacement PR.

## PR body

- Follow [`pull_request_template.md`](pull_request_template.md).
- Always link the issue: `Closes #…` for standalone issues, `Part of #…` for
  child PRs of umbrella/tracker issues (see [`CLAUDE.md`](../CLAUDE.md)
  § _Umbrella issue refs_).
- Label PRs only from the taxonomy in [`CLAUDE.md`](../CLAUDE.md) § _Labels_;
  never invent ad-hoc labels.

## Commits inside the PR

Merges are squash-only, so the PR title is what lands on `main`. Interior
commits may be informal during development; the final title is the contract.

## Everything else

For host-tool gateway rules, architecture boundaries, persistence
classification, firewalled-agent environment rules, `gh` quoting, and the
file-size / modularity bars, read [`CLAUDE.md`](../CLAUDE.md) and
[`AGENTS.md`](../AGENTS.md) before writing code.
