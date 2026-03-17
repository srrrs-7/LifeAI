# Frontmatter Fields Reference

## Skill Frontmatter (`.claude/skills/<name>/SKILL.md`)

### Supported fields

| Field | Required | Default | Description |
|---|---|---|---|
| `name` | No | Directory name | Lowercase, digits, hyphens only. Max 64 chars. Becomes the `/slash-command`. |
| `description` | Recommended | First paragraph of content | What the skill does + when to use it. Claude uses this to decide auto-loading. |
| `argument-hint` | No | (none) | Shown in autocomplete. E.g., `[issue-number]`, `[filename] [format]`. |
| `disable-model-invocation` | No | `false` | `true` = only user can invoke via `/name`. Claude cannot auto-trigger. |
| `user-invocable` | No | `true` | `false` = hidden from `/` menu. Only Claude can invoke. |

### NOT supported in skill frontmatter (causes warnings)

- ~~`model`~~ — Use a sub-agent with `model` instead
- ~~`context`~~ — Use the Agent tool in skill instructions instead
- ~~`agent`~~ — Use the Agent tool in skill instructions instead
- ~~`allowed-tools`~~ — Define in the sub-agent instead

### Invocation matrix

| Setting | User can invoke | Claude can invoke | Description loaded |
|---|---|---|---|
| (defaults) | Yes | Yes | Always in context |
| `disable-model-invocation: true` | Yes | No | NOT in context |
| `user-invocable: false` | No | Yes | Always in context |

---

## Agent Frontmatter (`.claude/agents/<name>.md`)

### Supported fields

| Field | Required | Default | Description |
|---|---|---|---|
| `name` | Yes | — | Unique identifier, lowercase + hyphens |
| `description` | Yes | — | When Claude should delegate to this agent |
| `tools` | No | All (inherited) | Tools the agent can use. E.g., `Read, Grep, Glob, Write, Edit, Bash` |
| `disallowedTools` | No | (none) | Tools to deny from inherited set |
| `model` | No | `inherit` | Model to use: `sonnet`, `opus`, `haiku`, or `inherit` |
| `permissionMode` | No | `default` | `default`, `acceptEdits`, `dontAsk`, `bypassPermissions`, `plan` |
| `maxTurns` | No | (unlimited) | Max agentic turns before stopping |
| `skills` | No | (none) | Skills to preload into agent context |
| `mcpServers` | No | (none) | MCP servers available to this agent |
| `hooks` | No | (none) | Lifecycle hooks scoped to this agent |
| `memory` | No | (none) | Persistent memory scope: `user`, `project`, `local` |
| `background` | No | `false` | `true` = always run as background task |
| `isolation` | No | (none) | `worktree` = run in isolated git worktree |

### Model options

| Value | Model |
|---|---|
| `opus` | Claude Opus 4.6 |
| `sonnet` | Claude Sonnet 4.6 |
| `haiku` | Claude Haiku 4.5 |
| `inherit` | Same as parent conversation (default) |

---

## Skill + Agent Pattern

For skills that need a specific model or heavy generation:

1. **Skill** (`.claude/skills/`) handles interactive conversation inline
2. **Agent** (`.claude/agents/`) handles generation with `model: opus`
3. Skill instructs Claude to use the Agent tool to delegate after gathering requirements
