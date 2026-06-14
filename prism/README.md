# prism

Prism is an early terminal harness for agent-backed coding sessions across Git worktrees.

Implemented so far:

- layered config from `~/.config/prism/config.toml` and repo `.prism.toml`
- `prism doctor`
- existing Git worktree discovery
- branch, prompt summary, adoption state, and Git status display including
  clean, dirty, ahead, behind, and diverged states
- first-run agent backend detection and default-agent selection
- a dependency-free two-panel TUI skeleton with Vim-style session navigation
- `c` create-session flow using `wt switch -c <branch>`
- dirty-worktree guard for session creation, overridable with `--allow-dirty`
- task prompt metadata under `.prism/tasks/<branch>.json`
- embedded PTY launch for the selected agent backend
- agent input mode with `i`/Enter and configurable `escape_key`
- built-in backend presets for `codex`, `pi`, `claude`, `opencode`, and `aider`
- custom command-template backends via config
- backend prompt delivery modes: `stdin`, `argument`, `temp-file`, and `interactive`
- live process state display: idle, running, done, failed, needs input, or needs restart
- lightweight process markers and raw agent logs under `.prism/`
- current-branch GitHub pull request detection with a right-side PR panel
- 10-second PR status/check polling through `gh pr view`
- cached PR summary refresh under `.prism/pr/`, with detail refresh when comments,
  reviews, checks, files, or head SHA change
- explicit `P` flow to push a clean branch and create a PR with `gh pr create --fill`
- optional configured checks for `pre_pr`, `pre_push`, and `review_fix`
- deterministic review packets under `.agent/review/<pr-number>.md`
- inline review comment fetch through `gh api`
- explicit review-fix agent launch, `fix: code review` commit, and push actions
- plan creation prompts that ask the selected agent to create or update `plans/<branch>.md`
- explicit plan execution through `codex-plan --file ...`
- remove-from-board and local delete flows for sessions

Install:

```sh
./install.sh
```

Run:

```sh
prism
prism --allow-dirty
prism --repo /path/to/repo
prism doctor
prism config
prism run-plan plans/my-branch.md
```

The TUI supports `c`, `i`/Enter, `n`, `x`, `P`, `R`, `f`, `m`, `u`, `a`, `D`,
`j`/`k`, arrow keys, `g g`, `G`, `r`, and `q`.

PR polling is read-only. Prism does not commit, push, or create a pull request
unless you press the matching explicit action and confirm the prompt. The `P`
flow refuses to continue when the selected worktree is dirty.

By default, `c` refuses to create a new worktree if the repository currently has
uncommitted changes. Start Prism with `--allow-dirty` to bypass that guard.

Review loop keys:

- `R` refreshes PR details and writes `.agent/review/<pr-number>.md`.
- `f` writes the review packet and starts a fresh agent session with it as context.
- `m` stages all changes and commits `fix: code review` after confirmation.
- `u` pushes the selected branch, confirming first when no upstream exists.

Plan keys:

- `n` starts the selected agent with a planning prompt for `plans/<branch>.md`.
- `x` runs the selected branch plan with `codex-plan` after confirmation.

Session cleanup keys:

- `a` hides the selected session from Prism without deleting Git data.
- `D` deletes Prism local data and, after confirmation, the local worktree and
  local branch. Remote branches and pull requests are not deleted.

Custom backend example:

```toml
default_agent = "my-agent"

[agents.my-agent]
command = "my-agent --prompt {prompt}"
prompt_mode = "argument"

[checks]
pre_pr = ["cargo test"]
pre_push = []
review_fix = []
```

Backend notes:

- Built-in `codex`, `pi`, `claude`, `opencode`, and `aider` adapters default to
  `stdin`, which starts the configured command in Prism's PTY and writes the
  initial prompt into it.
- `argument` appends the prompt as one argv value, or replaces `{prompt}` when
  present in the command template.
- `temp-file` writes the prompt to a temporary markdown file and appends that
  path, or replaces `{prompt_file}` when present.
- `interactive` starts the backend without sending a prompt.
- Prism does not translate prompts into backend-specific slash commands yet;
  configure `command` and `prompt_mode` per repo when a backend requires a
  different invocation style.
