# Context Hub — 知識抽出レポート (2026-04-03)

## 抽出された知識

| # | カテゴリ | 要約 | 詳細 | 信頼度 | 反映先セクション |
|---|---------|------|------|--------|----------------|
| 1 | decision | ACS Email をトランザクショナルメールに採用、SendGrid は将来のマーケティング用途として保留 | 招待・通知などの1対1送信は ACS Email（Managed Identity、Bicep完結、コスト安）で十分。マーケティングキャンペーン要件が具体化したタイミングで SendGrid を追加または移行する設計。両サービスは SPF レコードに `include:sendgrid.net` を追加するだけで共存可能。 | medium | Architecture |
| 2 | known_issue | `docker build --platform linux/amd64` は legacy builder でクロスコンパイル不可。`az acr build` を使う | Apple Silicon 等の arm64 ホストで `--platform linux/amd64` を指定すると `image does not provide the specified platform` エラー。`docker buildx` も古い Docker では `--name` フラグ非対応。回避策: `az acr build --registry <ACR> --image <img>:<tag> --platform linux/amd64 .` でサーバーサイドビルド。 | high | Known Issues（新規） |
| 3 | technical_insight | Playwright は `bun install` で入らない OS 依存ライブラリが必要。`sudo npx playwright install-deps` が必須 | ブラウザバイナリ（`bunx playwright install`）とは別に、libglib/libnss3 等の Linux システムライブラリが必要。これは npm/bun パッケージではなく `apt-get install` 相当。`bun install` では代替不可。 | medium | Known Issues（新規） |
| 4 | technical_insight | `e2e-auth-setup` は各ロール独立に `.auth/{role}.json` を保存。1ロールだけ再認証可能 | 認証セットアップは5ロール（顧客本部/グループ/店舗/所属なし/オペレーター）をそれぞれ独立保存する。失敗したロールだけ再実行すれば既存4件は上書きされない。タイムアウト時のデバッグ: `test-results/.../error-context.md` を確認。 | medium | Known Issues（新規） |
| 5 | technical_insight | Azure Blob Lifecycle Policy の `prefixMatch` は `コンテナ名/プレフィックス` 形式で指定 | `prefixMatch: ['crimo-csv-archive/']` でそのコンテナ内の全 Blob にマッチ。削除はフォルダ単位ではなく **Blob ごとの作成日時** 基準。同一フォルダでも作成日が異なれば削除タイミングが異なる。 | medium | Architecture |
| 6 | technical_insight | ACS Email のデフォルト送信制限は 30通/時。月1万通（≈14通/時）は申請不要 | 月1万通を均等分散すると約14通/時でデフォルト制限内。ただし一斉送信バーストは制限に抵触する可能性があるため、Worker 側でレート制限を実装して吸収する設計が必要。将来的に量が増えた段階で Azure サポートへ引き上げ申請。 | medium | Architecture |
| 7 | convention | バックアップ処理は既存バッチを変更せず独立した archiver ジョブとして実装 | Fileshare → Blob Storage へのアーカイブは `csv-archiver` ジョブを新規追加する方式。既存バッチ（CSV Creator, File Remover 等）にコード変更なし。Lifecycle Policy で30日後に自動削除。ジョブのスケジュールは File Remover より前に設定（例: 22:00 JST）。 | medium | Architecture |
| 8 | technical_insight | TanStack Query でキャッシュ読み取りのみ行う場合は `queryClient.getQueryData`、API 呼び出しあり＋キャッシュは `queryClient.fetchQuery` | `$get` を直接呼び出すとキャッシュが効かない。グループ検索はキャッシュ全件取得→クライアントフィルタ（`getQueryData`）、設置場所検索は毎回 API（`$get` 直呼び）という設計の使い分けが確認されている。 | low | Coding Conventions |

---

## CLAUDE.md への変更パッチ

```diff
--- a/CLAUDE.md
+++ b/CLAUDE.md
@@ -1,4 +1,4 @@
 # CLAUDE.md
 
 This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.
 
```

# reason: ACS Email 採用決定とレート制限の技術情報を Architecture セクションに追加
```diff
@@ -末尾付近（Dev Container セクションの後） @@
+
+## Known Issues & Workarounds
+
+### Docker クロスプラットフォームビルド（arm64 ホスト → linux/amd64）
+
+`docker build --platform linux/amd64` は legacy Docker builder ではクロスコンパイル不可（`image does not provide the specified platform` エラー）。`docker buildx` も古いバージョンでは `--name` フラグ非対応の場合がある。
+
+**回避策: ACR サーバーサイドビルドを使用**
+```bash
+az acr build --registry <ACR_NAME> --image <image>:<tag> --target <target> --platform linux/amd64 .
+```
+ソースコードを ACR に送信してクラウド側でビルドするため、ローカルアーキテクチャに依存しない。
+
+### Playwright OS 依存ライブラリ
+
+`bun install` では Playwright の OS 依存ライブラリ（libglib, libnss3 等）は入らない。Dev Container 初期化時は以下が必要:
+```bash
+bunx playwright install          # ブラウザバイナリ
+sudo npx playwright install-deps # OS 依存ライブラリ（apt-get 相当）
+```
+
+### E2E 認証セットアップ（ロール単位の再実行）
+
+`bun run e2e-auth-setup` は各ロールの認証を `.auth/{role}.json` に独立保存する。特定ロールのみ失敗した場合は再実行し、失敗したロールの認証情報のみ再入力すればよい（他ロールの `.json` は上書きされない）。認証タイムアウト時のデバッグ: `apps/e2e/test-results/.../error-context.md` を確認。
```

# reason: ACS Email アーキテクチャ情報を追記（レート制限・SendGrid共存）
```diff
@@ Architecture セクション内（または末尾） @@
+
+### メール送信アーキテクチャ（ACS Email）
+
+**採用方針**: トランザクショナルメール（招待・通知）は ACS Email を使用。Managed Identity 認証で API Key 管理不要、Bicep で IaC 完結。
+
+**SendGrid との使い分け**:
+- 現在: ACS Email のみ（トランザクショナルメール）
+- 将来マーケティングメール要件が確定した場合: SendGrid を追加（SPF レコードに `include:sendgrid.net` を追加するだけで ACS と共存可能）
+- Worker の送信部分を差し替えるだけで移行可能（Queue・DB・テンプレートは流用）
+
+**ACS Email レート制限**:
+- デフォルト: 30通/時（月1万通 ≈ 14通/時 → 申請不要）
+- 一斉送信バースト対策: Worker 側でレート制限を実装して吸収する
+- 将来的に量が増えた段階で Azure サポートへ引き上げ申請
+
+**バックアップアーカイブ設計**:
+- 既存バッチを変更せず、独立した `csv-archiver` ジョブ（Container App Job）で Fileshare → Blob Storage コピー
+- Blob Lifecycle Policy の `prefixMatch` は `コンテナ名/プレフィックス` 形式（例: `crimo-csv-archive/`）
+- 削除はフォルダ単位ではなく **Blob ごとの作成日時** 基準（`daysAfterCreationGreaterThan`）
+- archiver ジョブのスケジュールは File Remover より前に設定（例: 22:00 JST 実行）
```
