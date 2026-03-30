# Custom Env Vars - Documentation Research

## Existing Internal Documentation Inputs

- `docs/plans/custom-env-vars/feature-spec.md`
- `docs/plans/custom-env-vars/research-external.md`
- `docs/plans/custom-env-vars/research-business.md`
- `docs/plans/custom-env-vars/research-technical.md`
- `docs/plans/custom-env-vars/research-ux.md`
- `docs/plans/custom-env-vars/research-security.md`
- `docs/plans/custom-env-vars/research-practices.md`
- `docs/plans/custom-env-vars/research-recommendations.md`

## External References Informing This Feature

- Proton and Steam launch option behavior:
  - [Proton FAQ](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ)
  - [Proton debugging docs](https://raw.githubusercontent.com/ValveSoftware/Proton/proton_10.0/docs/DEBUGGING-LINUX.md)
- Launcher UX and env management references:
  - [Heroic env vars wiki](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Environment-Variables)
  - [Bottles CLI docs](https://docs.usebottles.com/advanced/cli)
- Process/env validity constraints:
  - [POSIX environment variable specification](https://pubs.opengroup.org/onlinepubs/9799919799.2024edition/basedefs/V1_chap08.html)
  - [Rust `Command::env` docs](https://doc.rust-lang.org/std/process/struct.Command.html#method.env)
- Accessibility guidance for editor UX:
  - [WAI-ARIA grid pattern](https://www.w3.org/WAI/ARIA/apg/patterns/grid/)
  - [WCAG focus visible](https://www.w3.org/WAI/WCAG22/Understanding/focus-visible)
  - [WCAG error identification](https://www.w3.org/WAI/WCAG21/understanding/error-identification.html)

## Required Documentation Deliverables

To call the feature complete, documentation must include:

- profile field schema details (`launch.custom_env_vars`),
- precedence rules between optimization and custom env values,
- reserved key restrictions and validation behavior,
- launch method parity statement (`proton_run`, `steam_applaunch`, `native`),
- troubleshooting examples for invalid keys and reserved-key rejection.
