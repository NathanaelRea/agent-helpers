# agent-helpers

A small collection of tools and skills for working with coding agents.

## Tools

- [`codex-plan`](./codex-plan/README.md) — run a phased markdown plan through `codex exec` one step at a time, with a split TUI for prompt history and current output.
- [`prism`](./prism/README.md) - early Rust TUI harness for discovering agent worktree sessions and checking local workflow prerequisites.

## Skills

Skills under [`.agents/skills/`](./.agents/skills/):

- [`architecture-survey`](./.agents/skills/architecture-survey/SKILL.md) — explore mode. Use when unsure what to build or whether simpler options exist. Maps the stack, names the problem, lays out 2–3 options neutrally.
- [`boring-architecture`](./.agents/skills/boring-architecture/SKILL.md) — pushback mode. Use when there's a specific proposal to add infrastructure (queue, worker, microservice, cache, etc.) and you want it stress-tested. Picks one option without hedging.
