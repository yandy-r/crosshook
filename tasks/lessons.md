# Lessons

## 2026-03-18

- When creating GitHub issues for this repo, do not assume `gh issue create --template ...` can be combined with `--body` or `--body-file`; the CLI rejects that combination.
- Do not assume YAML issue forms are discoverable by `gh issue create --template` in this repo. Validate first. If the CLI reports `no templates found`, use API/tooling to create a fully structured issue body that mirrors the intended form fields.
- When debugging missing `gh` tab completion for commands like `gh pr merge`, verify the shell and the CLI separately. If `_gh` is loaded but `gh __complete ...` returns no candidates, the problem is the CLI’s completion output, not shell wiring.
