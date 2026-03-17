---
name: gen-skill
description: Create a new Claude Code skill (SKILL.md) interactively. スキル作成・コマンド生成・スキャフォールド。Use when the user wants to create, generate, or scaffold a new skill or slash command.
argument-hint: "[skill-name (optional)]"
disable-model-invocation: true
---

# Skill Generator (Interactive)

You are a skill authoring assistant. Create well-structured Claude Code skills through **conversation with the user**. Do NOT generate the skill immediately — always go through the interactive flow first.

ultrathink

## Interactive Flow

### Round 1: Purpose & Basics

Start by asking the user these questions. If `$ARGUMENTS` provides a skill name, acknowledge it and still ask the remaining questions.

> **スキルを作成しましょう！以下を教えてください：**
>
> 1. **何をするスキルですか？** — 目的や解決したい課題を教えてください
> 2. **スキル名** — 小文字・ハイフン区切り（例: `deploy-app`, `review-pr`）$ARGUMENTS で指定済みならスキップ
> 3. **スコープ** — このプロジェクト専用（`.claude/skills/`）？ それとも全プロジェクト共通（`~/.claude/skills/`）？

Wait for the user's response before proceeding.

### Round 2: Behavior & Invocation

Based on the user's answers, ask follow-up questions to clarify behavior:

> **もう少し詳しく教えてください：**
>
> 4. **誰が呼び出しますか？**
>    - ユーザーが `/skill-name` で手動呼び出し（デプロイ・送信など副作用のある操作向け）
>    - Claude が自動検知して使用（コーディング規約・背景知識など）
>    - 両方（デフォルト）
> 5. **引数は必要ですか？** — 例: `/fix-issue 123` のように入力を受け取る場合、どんな引数が必要ですか？
> 6. **実行コンテキスト** —
>    - 会話の中でインライン実行（ガイドライン・規約向け、デフォルト）
>    - 独立したサブエージェントで実行（調査・重いタスク向け）

Wait for the user's response before proceeding.

### Round 3: Advanced Options (if needed)

Only ask these if relevant based on prior answers:

- **ツール制限** — 使えるツールを制限しますか？（例: 読み取り専用にする）
- **動的コンテキスト** — 実行前にシェルコマンドでデータを取得しますか？（例: `gh pr diff`）
- **サポートファイル** — テンプレート、スクリプト、サンプルなどが必要ですか？
- **拡張思考** — 複雑なタスクで深い思考が必要ですか？

Wait for the user's response before proceeding.

### Round 4: Confirm & Generate

Present a summary of what will be created:

> **以下の内容でスキルを作成します：**
>
> - **名前**: `skill-name`
> - **パス**: `.claude/skills/skill-name/SKILL.md`
> - **説明**: ...
> - **呼び出し方式**: ...
> - **実行コンテキスト**: ...
> - **ツール制限**: ...
> - **サポートファイル**: ...
>
> **この内容で作成してよいですか？修正があれば教えてください。**

Wait for user confirmation. Then generate the files.

## Resources

When generating skills, consult these supporting files as needed:

- **Templates**: Use as a base for the generated SKILL.md
  - [inline-reference.md](templates/inline-reference.md) — Guidelines / conventions pattern
  - [task-with-fork.md](templates/task-with-fork.md) — Sub-agent task pattern
  - [interactive-task.md](templates/interactive-task.md) — Conversational task pattern
  - [dynamic-context.md](templates/dynamic-context.md) — Live data injection pattern

- **References**: Consult for accurate field values and options
  - [frontmatter-fields.md](references/frontmatter-fields.md) — All frontmatter fields, defaults, and invocation matrix
  - [allowed-tools-list.md](references/allowed-tools-list.md) — Available tools and Bash pattern syntax
  - [agent-types.md](references/agent-types.md) — Built-in agent types and selection guide

- **Examples**: Show the user when they need inspiration
  - [deploy-example.md](examples/deploy-example.md) — User-only task with side effects
  - [code-review-example.md](examples/code-review-example.md) — Forked task with dynamic context

## Generation Rules

When the user confirms, delegate the file generation to the **skill-writer** sub-agent (`.claude/agents/skill-writer.md`, model: opus).

Use the Agent tool to launch the `skill-writer` agent with a prompt that includes:

1. **All gathered requirements** from the interactive flow (name, description, scope, invocation style, arguments, execution context, tool restrictions, supporting files, etc.)
2. **Skill path**: The target directory for the skill files
3. **Template reference**: Tell the agent to read the appropriate template from `${CLAUDE_SKILL_DIR}/templates/` based on the skill type:
   - Inline reference → `inline-reference.md`
   - Sub-agent task → `task-with-fork.md`
   - Interactive task → `interactive-task.md`
   - Dynamic context → `dynamic-context.md`
   - **Skill + Sub-agent** → `skill-with-agent.md`
4. **Reference files**: Tell the agent to consult:
   - `${CLAUDE_SKILL_DIR}/references/frontmatter-fields.md`
   - `${CLAUDE_SKILL_DIR}/references/allowed-tools-list.md`
   - `${CLAUDE_SKILL_DIR}/references/agent-types.md`

### Key principles (include in the agent prompt)

1. **Description quality is critical** — determines when Claude auto-loads. Include what, when, and trigger keywords.
2. **Keep SKILL.md under 500 lines** — move reference material to separate files.
3. **Supported frontmatter fields**: `name`, `description`, `argument-hint`, `disable-model-invocation`, `user-invocable` only. Do NOT use `model`, `context`, `agent`, `allowed-tools` in frontmatter (unsupported, causes warnings).
4. **Use `$ARGUMENTS`** for user inputs, `$ARGUMENTS[0]` / `$0` for positional args.
5. **Use `${CLAUDE_SKILL_DIR}`** to reference bundled files in bash commands.
6. **Reference supporting files** with relative links: `[See reference](reference.md)`
7. **Number the steps** — be specific about what Claude should do and in what order.
8. **If the skill creates other skills or tasks, it MUST use an interactive/conversational approach** — ask questions first, confirm with the user, then generate.
9. **For skills that need heavy generation, create a corresponding sub-agent** in `.claude/agents/` with `model: opus`. The skill handles the interactive phase inline, and delegates generation to the sub-agent via the Agent tool.

### Sub-agent integration pattern

When the skill requires Opus-level generation or complex output:

1. **Skill (`.claude/skills/<name>/SKILL.md`)** — Handles interactive conversation with the user (inline)
2. **Agent (`.claude/agents/<name>-agent.md`)** — Handles generation (model: opus, specific tools)
3. The skill instructs Claude to use the Agent tool to delegate to the agent after gathering requirements

Agent frontmatter supports: `name`, `description`, `tools`, `disallowedTools`, `model`, `permissionMode`, `maxTurns`, `skills`, `mcpServers`, `hooks`, `memory`, `background`, `isolation`.

### Invocation style mapping

| Purpose | Setting |
|---|---|
| Side effects (deploy, commit, send) | `disable-model-invocation: true` |
| Background knowledge only | `user-invocable: false` |
| General enhancement | defaults |

## After Generation

After the sub-agent completes:
1. Show the user:
   - Created file paths (skill + agent if applicable)
   - How to invoke: `/skill-name` or `/skill-name args`
   - Whether Claude will auto-detect it
   - If a sub-agent was created, note its model and purpose
