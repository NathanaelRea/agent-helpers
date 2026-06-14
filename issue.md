# Prism tmux/opencode prewarm attach regression

## Summary

Prism's tmux/opencode prewarm behavior is currently broken after the recent warm-up changes. Pressing Enter no longer reliably opens the selected worktree's opencode session, and the UI can show tmux lifecycle/status errors after opencode is killed.

The intended behavior is:

- Opening Prism should not block on tmux/opencode startup.
- Prism should prewarm a detached tmux session running opencode for each worktree.
- Pressing Enter on a worktree should attach instantly when the prewarmed session is ready.
- If opencode is killed with Ctrl-C and control returns to Prism, Prism should rewarm that same worktree shortly afterward.
- If the user waits about a second after returning to Prism, pressing Enter again should attach instantly to a fresh opencode session.
- Prism should not show transient tmux lifecycle errors such as `can't find session` as visible/persistent status failures.

## Current symptoms

Observed behavior while testing:

1. Fresh Prism launch: Enter can attach instantly in some cases.
2. Kill opencode with Ctrl-C and wait: Enter may attach instantly in some cases.
3. Kill opencode with Ctrl-C and quickly press Enter: attach becomes slow or broken.
4. Once in the broken state, later attempts can remain slow/broken for the rest of the Prism session.
5. After killing opencode, Prism can show a status like `can't find session...` when returning.
6. In the broken state, tmux appears to show a session/window name like `agent-helpers`, then later `agent-helpers.feat-prism`, then opencode eventually opens.
7. There may be a visual clue inside opencode: the sidebar/accent color is blue in the good state but red in the broken state.

## Recent changes involved

The recent work touched:

- `prism/src/tui.rs`
  - Added tmux warm-up channels and in-flight tracking.
  - Replaced synchronous tmux state refresh on startup/refresh/return paths with async warm-up.

- `prism/src/actions.rs`
  - Added background warm-up jobs.
  - Added selected-session warm-up waiting before attach.
  - Added delayed rewarm after returning from tmux.

- `prism/src/tmux.rs`
  - Added detached session creation for warm-up.
  - Added `detach-on-destroy on`.
  - Added stale-session replacement when a session exists but pane command is not opencode.
  - Added handling for transient `can't find session/window/pane` errors.

The current implementation may be too complex/racy. It likely needs simplification around ownership of a selected worktree's tmux session.

## Suspected failure areas

Likely causes to investigate:

- Race between foreground Enter attach and background warm-up for the same `TmuxWarmupKey`.
- Warm-up deciding a session is healthy too early while opencode is still exiting.
- Stale tmux sessions/windows/panes surviving after Ctrl-C and being attached before replacement completes.
- `attach-session` retry/recreate behavior fighting with delayed warm-up.
- Capturing or redirecting stdio for commands that need the real terminal. `tmux attach-session` must inherit the terminal.
- Prism status handling surfacing expected tmux lifecycle races as user-visible errors.

## Repro

1. Start Prism in this repo.
2. Select a worktree.
3. Press Enter.
4. Confirm opencode opens in the selected worktree.
5. Press Ctrl-C inside opencode so control returns to Prism.
6. Immediately press Enter again.
7. Observe whether attach is instant, slow, or broken.
8. Repeat after waiting about one second after Ctrl-C.
9. Watch for status-line errors like `can't find session...`.

## Desired fix direction

Prefer a simpler, explicit lifecycle:

- Only one code path should create/replace the tmux session for a worktree.
- Foreground attach should not race with background warm-up for the same worktree.
- Returning from tmux after Ctrl-C should schedule a reliable rewarm after tmux has settled.
- Transient missing-session errors during rewarm should be treated as normal and should not update the visible status line.
- Avoid any `tmux attach-session` invocation that pipes stdio; it must run attached to the user's terminal.

## Verification criteria

- `cargo test` passes.
- Manual:
  - Launch Prism; wait one second; Enter attaches instantly.
  - Ctrl-C opencode; wait one second; Enter attaches instantly to fresh opencode.
  - Ctrl-C opencode; immediately Enter; no permanent broken state.
  - No persistent `can't find session...` status after killing opencode.
  - Repeated kill/reattach cycles keep working.
