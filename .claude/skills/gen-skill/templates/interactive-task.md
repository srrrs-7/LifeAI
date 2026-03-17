---
name: <skill-name>
description: <What this skill does interactively. Include trigger keywords.>
disable-model-invocation: true
argument-hint: "[<optional-initial-arg>]"
---

# <Skill Title> (Interactive)

<One-line summary of purpose>

ultrathink

## Interactive Flow

### Round 1: Understand the goal

Ask the user:

> **<Greeting / context>. 以下を教えてください：**
>
> 1. **<Question about purpose>**
> 2. **<Question about scope/target>**
> 3. **<Question about preferences>**

Wait for the user's response before proceeding.

### Round 2: Clarify details

Based on the user's answers, ask follow-up questions:

> **もう少し詳しく教えてください：**
>
> 4. **<Detail question based on Round 1>**
> 5. **<Detail question based on Round 1>**

Wait for the user's response before proceeding.

### Round 3: Confirm & Execute

Present a summary:

> **以下の内容で進めます：**
>
> - **<Key 1>**: ...
> - **<Key 2>**: ...
> - **<Key 3>**: ...
>
> **この内容でよいですか？修正があれば教えてください。**

Wait for user confirmation. Then execute the task.

## Execution Rules

<Detailed instructions for what Claude should do after confirmation>
