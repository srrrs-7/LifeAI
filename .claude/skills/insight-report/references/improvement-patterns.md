# Improvement Patterns Reference

## Token Usage Patterns

### High cache miss rate (cache_hit_rate < 50%)
- **Symptom**: cache_creation >> cache_read
- **Cause**: Frequent context invalidation, short sessions, or misaligned prompts
- **Fix**: Consolidate CLAUDE.md instructions, reduce session restarts, use longer sessions

### Excessive input tokens
- **Symptom**: input_tokens consistently high relative to output
- **Cause**: Large files being read unnecessarily, verbose system prompts, or redundant context
- **Fix**: Trim CLAUDE.md, use targeted file reads with line ranges, split large skills

### High output tokens with low productivity
- **Symptom**: High output tokens but few tool calls or edits
- **Cause**: Claude is being overly verbose in explanations
- **Fix**: Add feedback memory or CLAUDE.md instruction for concise responses

## Tool Usage Patterns

### Over-reliance on Bash for search
- **Symptom**: Bash calls with grep/find/cat patterns
- **Cause**: Not using dedicated Read/Grep/Glob tools
- **Fix**: This is handled automatically by Claude, but CLAUDE.md can reinforce it

### Low tool diversity
- **Symptom**: Only a few tool types used across sessions
- **Cause**: Underutilization of available capabilities
- **Fix**: Suggest relevant tools/skills for common workflows

### Repeated failed tool calls
- **Symptom**: Same tool failing multiple times
- **Cause**: Permission issues, incorrect paths, or misconfigured tools
- **Fix**: Update permissions in settings.json, fix paths in skills

## Configuration Patterns

### Missing CLAUDE.md
- **Symptom**: No CLAUDE.md or empty CLAUDE.md
- **Cause**: Project not configured for Claude Code
- **Fix**: Generate CLAUDE.md with project conventions, build commands, test commands

### Unused skills
- **Symptom**: Skills exist in .claude/skills/ but are never invoked
- **Cause**: Poor descriptions, wrong invocation settings, or obsolete skills
- **Fix**: Update descriptions, remove obsolete skills

### Missing permissions
- **Symptom**: Frequent permission denials in logs
- **Cause**: settings.json allow list is too restrictive
- **Fix**: Add commonly approved patterns to settings.json permissions

## Session Patterns

### Very short sessions
- **Symptom**: Sessions with < 5 messages
- **Cause**: Frequent restarts, context loss
- **Fix**: Use longer sessions, leverage /compact for context management

### No memory utilization
- **Symptom**: No memory files in project memory directory
- **Cause**: Not using persistent memory feature
- **Fix**: Create relevant memory files for user preferences, project context

## Semantic Knowledge Patterns

Agent D が対話ログから検出する、CLAUDE.md に反映すべき知識パターン。

### Undocumented conventions
- **Symptom**: 同じコーディングパターンや規約が複数セッションで繰り返し説明されている
- **Cause**: 規約は暗黙的に存在するが CLAUDE.md に文書化されていない
- **Fix**: Coding Conventions セクションに該当規約を追加

### Repeated architecture explanations
- **Symptom**: アーキテクチャ上の意思決定やその理由が複数セッションで再説明されている
- **Cause**: 設計判断の根拠が CLAUDE.md に記録されていない
- **Fix**: Architecture セクションに意思決定とその理由を追記

### Recurring workarounds
- **Symptom**: 同じ回避策やワークアラウンドが複数回記述されている
- **Cause**: 既知の問題とその対処法が文書化されていない
- **Fix**: Known Issues セクション（新規作成）に問題と回避策を記載

### Repeated workflow instructions
- **Symptom**: 同じ作業手順やコマンドシーケンスをユーザーが繰り返し指示している
- **Cause**: 定型ワークフローが CLAUDE.md や Commands セクションに記載されていない
- **Fix**: Commands または Workflows セクションに手順を追加

### Undocumented technical constraints
- **Symptom**: 環境やライブラリの制約事項が対話で繰り返し言及されている
- **Cause**: プロジェクト固有の技術的制約が文書化されていない
- **Fix**: Architecture または Technical Notes セクションに制約事項を追記
