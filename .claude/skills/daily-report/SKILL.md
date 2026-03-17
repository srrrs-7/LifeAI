---
name: daily-report
description: Create a daily report (日報) interactively with text and infographic SVG. Use when the user wants to write a daily report, summarize their day, or create 日報.
argument-hint: "[date (optional, default: today)]"
disable-model-invocation: true
---

# Daily Report Generator (Interactive)

一日の出来事・気づき・次の一手を、テキスト日報とインフォグラフィックSVGにまとめるスキルです。5回の対話を通じてユーザーからヒアリングし、丁寧で読みやすい日報を作成します。

ultrathink

## Output

- **テキスト日報**: `${CLAUDE_SKILL_DIR}/assets/<yyyy-mm-dd>/daily.md`
- **インフォグラフィック**: `${CLAUDE_SKILL_DIR}/assets/<yyyy-mm-dd>/insights.svg`

日付は `$ARGUMENTS` で指定可能。省略時は今日の日付を使用します。

## Step 0: Preparation — Past Reports

Before starting the conversation, silently read past daily reports for context:

1. Use Glob to find existing reports: `${CLAUDE_SKILL_DIR}/assets/*/daily.md`
2. Read the **most recent 3 reports** (if they exist)
3. Take note of:
   - Previous "次の一手" items (to check progress)
   - Recurring themes or ongoing challenges
   - Related insights that may connect to today's report
4. Do NOT show this to the user yet — use it to enrich later questions and output

## Step 1: Hearing — 5 Interactive Questions

Ask questions **one at a time**. Wait for the user's response before asking the next question. Use a warm, polite tone (ですます調). Adapt follow-up phrasing based on previous answers.

If past reports exist and the previous report had "次の一手" items, weave a gentle follow-up into Q2 (e.g., "前回〇〇を予定されていましたが、そちらの進捗はいかがでしたか？").

### Q1: Highlight

> 🌟 **本日のハイライト**
>
> お疲れさまです！まず、今日一番印象に残った出来事を教えていただけますか？
> うまくいったこと、意外だったこと、何でも構いません。

### Q2: Activities

> 📋 **本日の取り組み**
>
> ありがとうございます。続いて、今日取り組まれたことを教えてください。
> 大きなことから小さなことまで、思いつくままにお願いします。
>
> (If past "次の一手" exists, add: 前回の日報で「〇〇」を予定されていましたが、そちらの進捗はいかがでしたか？)

### Q3: Insights

> 💡 **気づき・学び**
>
> ありがとうございます。今日新しく気づいたこと、学んだことはありますか？
> 技術的な発見、人との会話で得たヒント、ふとした閃きなど、どんなことでも大丈夫です。
>
> (If past insights relate to what the user mentioned, prompt: 以前「〇〇」という気づきがありましたが、今日のお話と関連がありそうですね。何か進展はありましたか？)

### Q4: Challenges

> 🚧 **課題・ブロッカー**
>
> 現在困っていること、詰まっていることはありますか？
> 技術的な問題でも、判断に迷っていることでも、お気軽にどうぞ。

### Q5: Next Steps

> 🎯 **次の一手**
>
> 最後に、明日以降にやりたいこと、次に取り組む予定のことを教えてください。
> 優先順位も添えていただけると助かります。

After Q5, respond with:

> ありがとうございます！いただいた内容をもとに、日報とインフォグラフィックを作成いたします。少々お待ちください。

## Step 2: Delegate to Sub-Agent for Generation

After all 5 questions are answered, delegate the generation work to the **daily-report-writer** sub-agent (`.claude/agents/daily-report-writer.md`, model: opus).

Use the Agent tool to launch the `daily-report-writer` agent with a prompt that includes:

1. **Date**: The target date (from `$ARGUMENTS` or today)
2. **Output directory**: `${CLAUDE_SKILL_DIR}/assets/<yyyy-mm-dd>/`
3. **Hearing results**: All 5 answers from the user, structured as:
   - Highlight: (Q1 answer)
   - Activities: (Q2 answer)
   - Insights: (Q3 answer)
   - Challenges: (Q4 answer)
   - Next Steps: (Q5 answer)
4. **Past report context**: Summary of relevant past reports (from Step 0)
5. **File references**: Tell the agent to read these for templates and design guidelines:
   - `${CLAUDE_SKILL_DIR}/templates/daily-template.md`
   - `${CLAUDE_SKILL_DIR}/templates/svg-design-guide.md`
6. **Instructions**: Generate both `daily.md` and `insights.svg` in the output directory

## Step 3: Finalize

After the sub-agent completes, show the user:
- File paths created
- A brief summary of the report content
- Suggest: "内容に修正があればお知らせください"
