# Agent Types Reference

Used with `context: fork` to specify which sub-agent executes the skill.

## Built-in Agents

### `Explore`
- **Purpose**: Fast, read-only codebase exploration
- **Tools**: Read, Grep, Glob (no write/edit)
- **Best for**: Research, code analysis, finding patterns, understanding architecture
- **Example use**: `deep-research`, `find-usage`, `analyze-dependencies`

### `Plan`
- **Purpose**: Architecture and implementation planning
- **Tools**: Read, Grep, Glob (no write/edit)
- **Best for**: Designing implementation strategies, identifying critical files, considering trade-offs
- **Example use**: `plan-refactor`, `design-feature`, `migration-plan`

### `general-purpose` (default)
- **Purpose**: Full-capability agent for autonomous tasks
- **Tools**: All tools available
- **Best for**: Tasks that need to read, write, execute, and modify
- **Example use**: `fix-issue`, `implement-feature`, `deploy`

## Custom Agents

Define custom agents in `.claude/agents/<name>.md`. Reference them by filename (without `.md`):

```yaml
context: fork
agent: my-custom-agent
```

## Choosing the Right Agent

| Need | Agent | Why |
|---|---|---|
| Search / read code | `Explore` | Fast, focused, no risk of modification |
| Plan before implementing | `Plan` | Structured analysis without side effects |
| Execute a full task | `general-purpose` | Needs write access and shell |
| Specialized workflow | custom agent | Tailored system prompt and tools |
