# Frontmatter Fields Reference

## Required / Recommended

| Field | Required | Default | Description |
|---|---|---|---|
| `name` | No | Directory name | Lowercase, digits, hyphens only. Max 64 chars. Becomes the `/slash-command`. |
| `description` | Recommended | First paragraph of content | What the skill does + when to use it. Claude uses this to decide auto-loading. |

## Invocation Control

| Field | Default | Description |
|---|---|---|
| `disable-model-invocation` | `false` | `true` = only user can invoke via `/name`. Claude cannot auto-trigger. Use for side-effect actions. |
| `user-invocable` | `true` | `false` = hidden from `/` menu. Only Claude can invoke. Use for background knowledge. |

### Invocation matrix

| Setting | User can invoke | Claude can invoke | Description loaded |
|---|---|---|---|
| (defaults) | Yes | Yes | Always in context |
| `disable-model-invocation: true` | Yes | No | NOT in context |
| `user-invocable: false` | No | Yes | Always in context |

## Execution

| Field | Default | Description |
|---|---|---|
| `context` | (inline) | `fork` = run in isolated sub-agent. Use for independent tasks. |
| `agent` | `general-purpose` | Sub-agent type when `context: fork`. Options: `Explore`, `Plan`, `general-purpose`, or custom agent name from `.claude/agents/`. |
| `model` | (inherited) | Override model: `sonnet`, `opus`, `haiku`. |
| `allowed-tools` | (all) | Comma-separated tool list. Grants these tools without per-use approval when skill is active. |

## UI / Help

| Field | Default | Description |
|---|---|---|
| `argument-hint` | (none) | Shown in autocomplete. E.g., `[issue-number]`, `[filename] [format]`. |

## Hooks

| Field | Default | Description |
|---|---|---|
| `hooks` | (none) | Lifecycle hooks scoped to this skill. See hooks documentation. |
