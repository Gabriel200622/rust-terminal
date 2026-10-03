# Resuming coding agents

On Unix desktops, local terminals started by Neptune add temporary `claude` and
`codex` adapters to their command search path. Launch either CLI normally. When
workspace restoration is enabled, closing and reopening Neptune opens each
remembered agent in its original pane and asks it to resume its recorded session
ID. Several panes can use the same directory without selecting each other's
latest conversation. There is no `--last` or directory-history guessing.

For Codex, review and trust Neptune's **SessionStart** hook when Codex asks.
Its command is the Neptune executable followed by `--agent-hook codex`. Existing
hooks remain installed; Neptune does not bypass their trust checks. If the hook
was skipped, trust it in `/hooks` and restart Codex to capture subsequent
sessions. Codex versions exposing `--no-daemon` run locally so hooks belong to
this terminal, independently of a shared server's environment. Older versions
still open normally, but automatic exact-session capture is unavailable.

Claude Code receives an additional invocation-scoped SessionStart hook through
`--settings`. Neither integration rewrites shell dotfiles or the providers'
global settings. Bash and Zsh startup adapters load the user's configuration
before adding the command adapters; other Unix shells, and a shell configured
with its own arguments, use the inherited PATH.
Aliases, functions, absolute executable paths, another shell that replaces PATH,
and user-supplied hook/settings overrides can bypass capture.

If an agent has not reported a usable ID (including disabled or untrusted hooks, or an untouched Codex prompt),
Neptune reopens that CLI at its recorded directory. It cannot guarantee the same
conversation in that case. Exact resumption also requires the provider's local
session history to remain available. An unsuccessful resume keeps the saved
reference and leaves a shell available; **Restart terminal** clears the reference.
Closing an agent normally clears its reference. Closing a pane or workspace
removes it; changing SSH hosts clears it; cancelling a close leaves it intact.
A split starts a fresh shell. Hidden workspaces still receive hook updates.

`--command` takes precedence in its target pane and starts a shell there.
`--no-restore` and `restore_workspaces = false` retain their existing meaning.
Neptune restores new processes, not unfinished tool execution or process memory.
Only provider, session ID and directory are saved in workspace schema 6, which
reads versions 1–5. Invalid references receive the same recovery-copy protection
as other damaged workspace state. Prompts, transcripts, arbitrary commands,
credentials and permission-bypass flags are not saved or replayed. Transcripts
remain owned by the CLI. Launch-only options and temporary environment changes
(such as a different `CODEX_HOME`) must be reapplied by the user's configuration.

This integration is for local interactive CLIs. Batch/print commands, nested
agents, SSH sessions and Windows shells do not participate. An application update
cannot discover agents launched before its adapters were installed. macOS uses
the Unix adapter but still needs native verification; Linux/X11 is the verified
host for this change.

## Implementation and source review

The pure model validates resume references and accepts generation-tagged
`PaneAgentChanged` commands. Persistence stores a reference on each pane. The
desktop startup workers install private adapters, and a single blocking loopback
listener receives bounded metadata messages. Each pane generation has a random
callback credential, and each CLI invocation has its own identity. Closed or
replaced panes and late hooks from exited invocations cannot overwrite a newer
session. Pending updates coalesce per pane; hooks wake the application on change,
so idle integrations do not poll or repaint. Socket reads have byte and total-time
limits. Startup-file creation and cleanup stay on workers.

A small POSIX supervisor preserves foreground signal handling and reports normal
CLI exit. Resumption runs as part of shell startup with quoted arguments, never
by typing commands into a terminal that could still be showing another prompt.
No backend terminal-engine types or transcript readers were added.

Reviewed against the installed **Codex CLI 0.160.0** and **Claude Code 2.1.287**,
and the providers' documentation on 2026-10-02:

- [Codex CLI](https://learn.chatgpt.com/docs/codex/cli) and local `codex resume --help` describe targeted resumption.
- [Codex hooks](https://learn.chatgpt.com/docs/hooks) describes SessionStart metadata, additive hook sources and trust review.
- [Claude Code CLI reference](https://code.claude.com/docs/en/cli-reference) describes `--resume` and invocation settings.
- [Claude Code hooks](https://code.claude.com/docs/en/hooks) describes session-start metadata and lifecycle changes.

## Focused verification

`python3 scripts/verify-agent-restore.py` runs deterministic CLI fixtures in an
isolated native app, with fresh storage and a unique inspection endpoint. It
checks independent IDs in one directory, graceful close/reopen, normal exit,
Ctrl+C and narrow-window presentation. Fixtures prove Neptune's contracts without
sending model requests; they do not stand in for installed-provider validation.
Build the inspection targets described in [native verification](verification.md#native-application-verification) first.

The implementation was also checked in the running Linux/X11 app with the two
installed CLIs above. Each received a no-tools request to reply with `OK`; both
conversations and their exact IDs returned after a graceful close and reopen.
The development build with `inspection` was reviewed at 1100×700 and 640×440.
The deterministic native regression additionally verifies normal agent exit and
Ctrl+C. These checks establish Linux behavior, not macOS/Windows readiness or
performance. Focused model, persistence, runtime and application tests, desktop
library Clippy and the architecture boundary check passed.
