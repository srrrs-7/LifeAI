# Example: PR Review Skill

A forked task with dynamic context injection.

## SKILL.md

````yaml
---
name: pr-review
description: Review the current pull request with security and quality checks
context: fork
agent: Explore
allowed-tools: Bash(gh *), Read, Grep, Glob
disable-model-invocation: true
---

# PR Review

## Live Context

- PR description: !`gh pr view --json title,body --jq '.title + "\n\n" + .body'`
- Diff: !`gh pr diff`
- Changed files: !`gh pr diff --name-only`
- CI status: !`gh pr checks`

## Review Checklist

Analyze the diff above and check for:

### Security
- [ ] No hardcoded secrets or credentials
- [ ] Input validation on user-facing endpoints
- [ ] No SQL injection or XSS vulnerabilities

### Quality
- [ ] Error handling for edge cases
- [ ] No obvious performance issues (N+1 queries, unbounded loops)
- [ ] Consistent naming and code style

### Tests
- [ ] Changed code has corresponding test coverage
- [ ] Tests cover both happy path and error cases

## Output

Provide a structured review with:
1. **Summary** — one paragraph overview
2. **Issues** — list with severity (critical/warning/info) and file:line references
3. **Suggestions** — optional improvements
````

## Why this works

- `context: fork` + `agent: Explore` — isolated read-only analysis
- `!`command`` — fetches live PR data before Claude sees the prompt
- Structured checklist ensures consistent review quality
