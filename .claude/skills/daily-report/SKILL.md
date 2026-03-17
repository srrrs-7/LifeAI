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

## Step 2: Generate daily.md

Create `${CLAUDE_SKILL_DIR}/assets/<yyyy-mm-dd>/daily.md` following the template in [daily-template.md](templates/daily-template.md).

### Writing guidelines

- **対象読者は不特定多数** — 専門用語には簡潔な補足を添える
- **ですます調** で統一。堅すぎず、砕けすぎない丁寧な文体
- **一文一義** — 1つの文に1つの情報
- **具体的に** — 数値、固有名詞、事実を優先。曖昧な表現を避ける
- **ハイライトは冒頭に** — 最も重要な情報を最初に置く（逆ピラミッド構造）
- **箇条書きを活用** — 長文パラグラフより構造化されたリストを優先
- **過去日報との接続** — 前回の「次の一手」の進捗、繰り返し出現するテーマ、関連する過去の気づきを自然に織り込む

## Step 3: Generate insights.svg

Create `${CLAUDE_SKILL_DIR}/assets/<yyyy-mm-dd>/insights.svg` as an infographic. Follow the design guidelines in [svg-design-guide.md](templates/svg-design-guide.md).

### SVG Infographic Requirements

The SVG must provide **a single-glance overview of the entire day**. It should contain:

1. **Header**: Date and highlight of the day
2. **Activities Section**: Icon-based list or flow of what was done
3. **Insights Section**: Key learnings with visual emphasis (lightbulb icons, highlight colors)
4. **Challenges Section**: Current blockers with status indicators
5. **Next Steps Section**: Action items with priority indicators
6. **Connection to Past**: Visual thread showing continuity from previous reports (if applicable)

### SVG Technical Constraints

- Standalone SVG file (no external dependencies)
- Width: 800px, Height: auto (scale to content, typically 600-1200px)
- UTF-8 encoding, embedded fonts (system-ui fallback)
- All text must be readable at 100% zoom
- Use `<text>`, `<rect>`, `<circle>`, `<line>`, `<path>` — no `<foreignObject>`
- Embed emoji/icons as text characters (🌟💡🚧🎯📋) or draw simple SVG icons

## Step 4: Finalize

1. Create directory: `${CLAUDE_SKILL_DIR}/assets/<yyyy-mm-dd>/`
2. Write `daily.md`
3. Write `insights.svg`
4. Show the user:
   - File paths created
   - A brief summary of the report
   - Suggest: "内容に修正があればお知らせください"
