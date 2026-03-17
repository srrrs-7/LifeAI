---
name: <skill-name>
description: <What this skill does with live data. Include trigger keywords.>
context: fork
allowed-tools: <tool-list>
disable-model-invocation: true
---

# <Task Title>

## Live Context

<Use !`command` to inject dynamic data before Claude sees the prompt.>
<These commands run BEFORE Claude processes the skill — Claude only sees the output.>

- Current branch: !`git branch --show-current`
- Recent commits: !`git log --oneline -5`
- Changed files: !`git diff --name-only`

## Your Task

Based on the context above:

1. <Step one>
2. <Step two>
3. <Step three>

## Output format

<What the final output should look like>
