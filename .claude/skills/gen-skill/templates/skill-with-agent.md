---
name: <skill-name>
description: <What this skill does interactively + WHEN to use it. Include trigger keywords.>
disable-model-invocation: true
argument-hint: "[<optional-arg>]"
---

# <Skill Title> (Interactive)

<One-line summary of purpose>

ultrathink

## Output

- **Output file 1**: `${CLAUDE_SKILL_DIR}/assets/<path>/<file1>`
- **Output file 2**: `${CLAUDE_SKILL_DIR}/assets/<path>/<file2>`

## Step 0: Preparation

<Optional: silently read past data, context, or references before starting conversation>

## Step 1: Interactive Hearing

Ask questions **one at a time**. Wait for the user's response before asking the next.

### Q1: <Topic>

> <Emoji> **<Question title>**
>
> <Polite question text>

### Q2: <Topic>

> <Emoji> **<Question title>**
>
> <Polite question text>

### Q3: <Topic>

> <Emoji> **<Question title>**
>
> <Polite question text>

(Add more questions as needed)

After all questions, respond with:

> ありがとうございます！いただいた内容をもとに作成いたします。少々お待ちください。

## Step 2: Delegate to Sub-Agent

Delegate the generation work to the **<skill-name>-writer** sub-agent (`.claude/agents/<skill-name>-writer.md`, model: opus).

Use the Agent tool to launch the agent with a prompt that includes:

1. **All hearing results**: Structured summary of user's answers
2. **Output directory**: `${CLAUDE_SKILL_DIR}/assets/<path>/`
3. **Context**: Any past data or references gathered in Step 0
4. **File references**: Tell the agent to read templates/guides from `${CLAUDE_SKILL_DIR}/templates/`
5. **Instructions**: What files to generate and where

## Step 3: Finalize

After the sub-agent completes, show the user:
- File paths created
- A brief summary of the output
- Suggest: "内容に修正があればお知らせください"
