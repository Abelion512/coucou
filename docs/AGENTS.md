# Coucou — third-party agent integration

Any tool that can write to a per-user Unix socket can send events to Coucou and have
its own pill next to Claude Code. This is the Linux fork; upstream's macOS and Windows
notes (Keychain, `nb-hook`, named pipes) do not apply here.

## The `coucou_agent` field

Add the optional field `coucou_agent` to any hook JSON payload. Coucou creates a pill
labelled with the agent name and routes all its events to it.

**Validation:** the name must match `^[a-z0-9-]{1,24}$` (lowercase letters, digits and
hyphens, 1–24 characters) and must not be `claude`, which stays reserved for the Claude
Code pill. An absent or invalid name routes the event to the Claude Code pill instead.

## Hook command

Call the relay with `--agent <your-name>` after the event name:

```json
{
  "hooks": {
    "UserPromptSubmit": [
      { "type": "command", "command": "$HOME/.local/share/coucou/bin/coucou-hook --agent my-tool UserPromptSubmit" }
    ]
  }
}
```

The relay parses argv, injects `coucou_agent` into the payload and forwards it. Absent
`--agent`, the payload is sent untouched and stays on the Claude Code pill.

## Payload format

Newline-terminated JSON to `$XDG_RUNTIME_DIR/coucou/coucou.sock`
(fallback `/tmp/coucou-<uid>.sock`). The socket is `SO_PEERCRED`-checked, so only your
own user can connect. You can add `coucou_agent` yourself:

```json
{
  "hook_event_name": "UserPromptSubmit",
  "session_id": "my-session-1",
  "coucou_agent": "my-tool",
  "prompt": "Running task…"
}
```

## Supported events

All standard Claude Code hook events are supported, **except `PermissionRequest`**:
approval cards are Claude-Code-only (see the fork's guardrails), so a
`PermissionRequest` from an external agent is declined immediately — the relay writes
no decision and the agent re-asks in its own terminal.

The pill lifecycle:

| Event | Effect |
|---|---|
| `SessionStart` | Creates the pill (if absent), sets state to idle |
| `UserPromptSubmit` | State → thinking; prompt shown in ticker |
| `PreToolUse` | State → working; tool label shown in ticker |
| `PostToolUse` / `PostToolUseFailure` | State → working |
| `Notification` | Rate-limit or question state if applicable |
| `Stop` | State → finished for 5 s, then pill removed |
| `StopFailure` | State → error |
| `SessionEnd` | Pill removed |
| `SubagentStart` / `SubagentStop` | Step added to ticker |

## The adapters this fork ships

OpenCode, Hermes and Freebuff/Codebuff are watched by three observe-only adapters in
`app/src-tauri/src/agents/` (SSE, gateway control socket, file watching). They need no
hooks: point them at loopback and their sessions light up a pill on their own. The
`coucou_agent` field above is for anything else you want to plug in.

## Quick test

With Coucou running:

```sh
echo '{"hook_event_name":"UserPromptSubmit","session_id":"t1","prompt":"hello","coucou_agent":"demo"}' \
  | "$HOME/.local/share/coucou/bin/coucou-hook" --agent demo
```

A "demo" pill should appear in the island. If Coucou is not running the relay exits 0
immediately — the tool is never blocked.