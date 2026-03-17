# Allowed Tools Reference

## Core Tools

| Tool | Description |
|---|---|
| `Read` | Read file contents |
| `Write` | Create or overwrite files |
| `Edit` | Make targeted edits to existing files |
| `Glob` | Find files by pattern (e.g., `**/*.ts`) |
| `Grep` | Search file contents by regex |
| `Bash` | Execute shell commands |
| `Agent` | Launch sub-agents for complex tasks |
| `Skill` | Invoke other skills |
| `WebFetch` | Fetch and process web content |
| `WebSearch` | Search the web |
| `NotebookEdit` | Edit Jupyter notebooks |

## Bash Pattern Syntax

`Bash` supports glob patterns to restrict which commands are allowed:

```yaml
allowed-tools: Bash(git *)        # Only git commands
allowed-tools: Bash(npm *)        # Only npm commands
allowed-tools: Bash(python *)     # Only python commands
allowed-tools: Bash(gh *)         # Only GitHub CLI
allowed-tools: Bash(git *), Bash(npm *)  # Multiple patterns
```

## Common Presets

### Read-only exploration
```yaml
allowed-tools: Read, Grep, Glob
```

### Read + shell (restricted)
```yaml
allowed-tools: Read, Grep, Glob, Bash(git *), Bash(npm test *)
```

### Web research
```yaml
allowed-tools: Read, Grep, Glob, WebFetch, WebSearch
```

### Full code modification
```yaml
allowed-tools: Read, Write, Edit, Grep, Glob, Bash(git *), Bash(npm *)
```

### GitHub workflow
```yaml
allowed-tools: Read, Grep, Glob, Bash(gh *), Bash(git *)
```
