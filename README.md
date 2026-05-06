# codex-plan

- Minimal helper for running a plan broken up with phases through `codex exec`, one step at a time.
- Rationale:
  - break large work into focused plan phases
  - minimize unnecessary or potentially misleading context
  - drill into each phase separately so Codex can spend its attention on the right slice of the problem
- Selects a `*.md` plan file from the current repo with `fzf`.
- Prompts for:
  - step name, defaulting to `phase`
  - total step count
  - starting step
- Runs each step as:

  ```sh
  codex exec "Implement <plan-file> <step-name> <step-number>"
  ```

- Shows a split TUI when attached to a real terminal:
  - left pane: prompt history for this run
  - right pane: current Codex output
  - line cursor and current step status
  - completed steps include the last context/token summary found in Codex output
- Supports Vim-style output navigation:
  - `?`: show or hide the shortcuts tooltip
  - `j` / down: move down
  - `k` / up: move up
  - `gg`: jump to top
  - `G`: jump to bottom and follow new output
  - `{` / `}`: jump between collapsed or expanded blocks
  - space / enter: expand or collapse the current block
- Collapses noisy tool output:
  - tool-call blocks are shown as one-line summaries until expanded
  - collapsed summaries include the block start timestamp and output line number
  - diff hunks are shown as one-line summaries until expanded
  - expanded diffs highlight added, deleted, and metadata lines
  - long output is kept in a bounded line buffer with an omission marker when old lines are dropped
- Falls back to plain sequential output when a TUI is not available.
- Install with:

  ```sh
  ./codex-plan/install.sh
  ```

- Installs a symlink to `~/.local/bin/codex-plan` by default.
- Override the install directory with:

  ```sh
  CODEX_PLAN_INSTALL_DIR=/path/to/bin ./codex-plan/install.sh
  ```

- Requires `codex`, `fzf`, and Python 3.
