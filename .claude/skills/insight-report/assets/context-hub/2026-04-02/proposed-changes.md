# Context Hub — 知識抽出レポート (2026-04-02)

## 抽出された知識

| # | カテゴリ | 要約 | 詳細 | 信頼度 | 反映先セクション |
|---|---------|------|------|--------|----------------|
| 1 | decision | ACS Email を SendGrid より優先し、マーケティング要件発生時に SendGrid を追加導入する方針 | トランザクショナルメールは ACS（Managed Identity・コスト・Azure統合で優位）、マーケティングメールが必要になったら SendGrid を追加（共存可能）。Worker の送信部分を差し替えるだけで移行可能な設計。 | medium | — (HOSHIONE プロジェクト固有) |
| 2 | technical_insight | Docker legacy builder は `--platform linux/amd64` のクロスビルドに対応できない。`az acr build` でサーバーサイドビルドが有効 | arm64 ホストから amd64 イメージを作る際、legacy builder では `image was found but does not provide the specified platform` エラー。`docker buildx` または `az acr build --platform linux/amd64` で解決。ACR ビルドはローカルアーキテクチャに依存しない。 | high | — (Azure 運用知見) |
| 3 | technical_insight | Azure Blob Lifecycle Policy は個々の Blob の作成日基準で動作し、フォルダ単位ではない | `daysAfterCreationGreaterThan: 30` は各 Blob の作成日時を参照。同一フォルダ内でも作成日時が異なれば削除タイミングは異なる。`prefixMatch` は `コンテナ名/プレフィックス` 形式で指定。 | medium | — (Azure 運用知見) |
| 4 | technical_insight | Playwright は「ブラウザバイナリ」と「OS依存ライブラリ」の2段階インストールが必要 | `bunx playwright install`（Chromium等）と `sudo npx playwright install-deps`（libglib, libnss3等の apt パッケージ）は別物。後者は `bun install` では入らない。 | medium | — (E2E テスト知見) |
| 5 | decision | バッチアプリのバックアップは既存コード無変更で、専用 archiver ジョブを追加する方式を採用 | Fileshare → Blob コピーを行う軽量ジョブ（csv-archiver）を新規追加し、既存バッチ群は変更なし。Lifecycle Policy で30日後に自動削除。File Remover の実行前に archiver が動くようスケジュール調整が必要。 | medium | — (HOSHIONE プロジェクト固有) |
| 6 | convention | ジョブ命名で `remover` より `archiver` が適切（主目的がアーカイブの場合） | h2c-remover → h2c-archiver に改名。英語として自然で、「Blobにアーカイブして古いものを片付ける」という役割を正確に表現。`backuper` は造語なので避ける。 | low | — (命名規約) |
| 7 | workflow | E2E テストは feature/シナリオ単位で実行可能 | `bun run generate && npx playwright test --grep "パターン"` で feature 単位・シナリオ単位のテスト実行が可能。認証セッション期限切れ時は `bun run e2e-auth-setup` で更新。 | medium | — (E2E テスト運用) |
| 8 | decision | 開発者のナレッジと会話ログの蓄積から有用なコンテキストを抽出・配布する仕組みが SIer に必要 | AI時代において「経験ナレッジの配布」が単価向上に繋がる。アイデアで勝負するかブランドで勝負するかの2択。ログデータの先行収集が将来価値を生む（活用方法を思いついた時点でのポジションが勝負を分ける）。 | medium | — (プロジェクト背景・ビジョン) |
| 9 | known_issue | サブエージェントが Bash 権限不足で失敗した場合、メインプロセスでフォールバック実行が必要 | insight-report の並列エージェント調査で、App 層エージェントが Bash ツール権限不足で Azure CLI コマンドを実行できず失敗。メインプロセスで直接実行するフォールバック戦略が有効。 | medium | Architecture > スキル・エージェント設計パターン |
| 10 | technical_insight | TanStack Query の `fetchQuery` と直接 API 呼び出しではキャッシュ挙動が異なる | `queryClient.fetchQuery` は queryKey 一致でキャッシュヒットするが、直接 `$get` を呼ぶとキャッシュされない。検索結果のキャッシュ再利用には `fetchQuery` + queryKey に検索文字列を含める設計が必要。 | low | — (フロントエンド知見) |

## CLAUDE.md への変更パッチ

対話ログから得られた知識のほとんどは HOSHIONE プロジェクト固有または一般的な技術知見であり、lifeai プロジェクトの CLAUDE.md に直接反映すべき新規事項は限定的です。

# reason: サブエージェントが権限不足で失敗するケースの対処法を設計パターンに追記
```diff
--- a/CLAUDE.md
+++ b/CLAUDE.md
@@ -42,6 +42,8 @@
 
 1. **スキル（`.claude/skills/<name>/SKILL.md`）** — ユーザーとの対話型ヒアリング（インライン実行）
 2. **サブエージェント（`.claude/agents/<name>-writer.md`）** — ヒアリング結果をもとに成果物を生成（model: opus）
+
+**サブエージェントのフォールバック**: サブエージェントが Bash 権限不足等で失敗した場合、メインプロセスで直接実行するフォールバック戦略を取る。並列エージェント実行時は失敗したエージェントのタスクのみメインプロセスに引き取り、他のエージェントの完了を待つ。
 
 各スキルは `templates/` にテンプレート、`references/` にリファレンス、`assets/` に成果物を格納。
```
