# prism

Prism is an early terminal harness for agent-backed coding sessions across Git worktrees.

Implemented so far:

- layered config from `~/.config/prism/config.toml` and repo `.prism.toml`
- `prism doctor`
- startup checks for required `git`, `gh`, and configured worktree command
- existing Git worktree discovery
- branch, prompt summary, adoption state, and Git status display including
  clean, dirty, ahead, behind, and diverged states
- OpenCode backend detection
- a dependency-free two-panel TUI skeleton with Vim-style session navigation
- `c` create-session flow using `wt switch -c <branch>`
- dirty-worktree guard for session creation, overridable with `--allow-dirty`
- task prompt metadata in the per-repo SQLite database
- tmux-backed persistent OpenCode TUI sessions with Enter attach
- embedded PTY launch for prompt-driven OpenCode actions
- OpenCode JSON event output for prompt-driven actions
- optional OpenCode command-template overrides via config
- backend prompt delivery modes: `stdin`, `argument`, `temp-file`, and `interactive`
- live process state display: idle, running, done, failed, needs input, or needs restart
- lightweight process state in SQLite and raw agent logs under `~/.config/prism/repos/<repo>/logs/`
- persistent TUI status line and action failure log under `~/.config/prism/repos/<repo>/runtime.log`
- current-branch GitHub pull request detection with a right-side PR panel
- 15-second repo-wide PR summary/check polling through GitHub GraphQL
- cached PR summary refresh in SQLite, with selected-PR detail refresh at most
  every 30 seconds
- explicit `P` flow to push a clean branch and create a PR with `gh pr create --fill`
- optional configured checks for `pre_pr`, `pre_push`, and `review_fix`
- deterministic review packets under `.agent/review/<pr-number>.md`
- inline review comment fetch through `gh api`
- explicit review-fix prompt staging, review-fix commit, and push actions
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

Required tools for the TUI are `git`, `gh`, `tmux`, the configured worktree
command (`wt` by default), and `opencode`. `codex-plan` is optional and only
needed for the temporary `x`/`run-plan` path.

On startup, Prism checks the repository layout before opening the board. If the
current checkout is not on the default branch, no additional worktree sessions
are set up yet, Prism shows a setup prompt. From that prompt you can open anyway
or, on a clean non-default branch checkout, move the branch into a Worktrunk
worktree and switch the original checkout back to the default branch.

The TUI supports `?` for the full keybinding dialog.

Pressing Enter attaches to a persistent tmux session for the selected worktree.
If that worktree does not have an agent tmux session yet, Prism starts
`opencode` in the worktree directory. Detach from tmux, for example with
`Ctrl-b d`, to return to Prism. Other worktree agent sessions keep running in
parallel while detached. Prism only treats the tmux session as running while the
pane is actually running OpenCode.

PR polling is read-only. Prism does not commit, push, or create a pull request
unless you press the matching explicit action and confirm the prompt. The `P`
flow refuses to continue when the selected worktree is dirty.

By default, `c` refuses to create a new worktree if the repository currently has
uncommitted changes. Start Prism with `--allow-dirty` to bypass that guard.
When creation is allowed, `c` prompts on the bottom line for `Branch name:`,
then `Initial prompt (optional):`; it does not open a modal dialog.

Runtime diagnostics:

- `~/.config/prism/repos/<repo>/logs/<branch>.log` stores raw agent output for a branch.
- `~/.config/prism/repos/<repo>/runtime.log` stores Prism action failures shown in the TUI status line.
- tmux agent sessions are named `prism-<repo-hash>-<branch>`.
- PR refresh errors are cached and shown in the PR panel.

Review loop keys:

- `R` refreshes PR details and writes `.agent/review/<pr-number>.md`.
- `f` refreshes PR comments, creates the interactive agent session if needed,
  and pastes the comment-only review-fix prompt without submitting it.
- `m` stages all changes and commits with an editable default message of
  `fix: code review`.
- `u` pushes the selected branch, confirming first when no upstream exists.

Plan keys:

- `n` starts the selected agent with a planning prompt for `plans/<branch>.md`.
- `x` runs the selected branch plan with `codex-plan` after confirmation. This
  is a temporary bridge; the long-term plan runner belongs inside Prism as
  Ralph.

Session cleanup keys:

- `a` hides the selected session from Prism without deleting Git data.
- `D` deletes Prism local data and, after confirmation, the local worktree and
  local branch. Remote branches and pull requests are not deleted.

OpenCode command override example:

```toml
default_agent = "opencode"

[agents.opencode]
command = "opencode run --format json"
prompt_mode = "argument"

[checks]
pre_pr = ["cargo test"]
pre_push = []
review_fix = []
```

Backend notes:

- Prism uses OpenCode as its agent backend. Prompt-driven actions default to
  `opencode run --format json <prompt>` so Prism can capture structured event
  output.
- The Enter tmux attach path starts `opencode` directly without sending an
  initial prompt, regardless of the prompt-driven command override.
- `argument` appends the prompt as one argv value, or replaces `{prompt}` when
  present in the command template.
- `temp-file` writes the prompt to a temporary markdown file and appends that
  path, or replaces `{prompt_file}` when present.
- `interactive` starts the backend without sending a prompt.
- Prism does not translate prompts into OpenCode-specific slash commands yet;
  configure `agents.opencode.command` and `agents.opencode.prompt_mode` per repo
  when a different invocation style is needed.
