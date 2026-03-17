# LifeAI

個人の活動レコード（日報・振り返り・気づき等）を残していくための AI 対話フレームワーク。
Claude Code のスキルシステムを活用し、対話を通じて個人のライフマネジメントを支援します。

## Getting Started

### 前提条件

- [Docker](https://www.docker.com/) & [Docker Compose](https://docs.docker.com/compose/)
- [VS Code](https://code.visualstudio.com/) + [Dev Containers 拡張機能](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)

### セットアップ

```bash
# 1. リポジトリをクローン
git clone <repository-url>
cd lifeai

# 2. VS Code で Dev Container を起動
#    VS Code でフォルダを開き、コマンドパレットから
#    「Dev Containers: Reopen in Container」を実行

# 3. Git hooks をインストール
make hooks

# 4. ビルド確認
make build

# 5. テスト実行
make test
```

### よく使うコマンド

| コマンド | 説明 |
|---------|------|
| `make build` | ビルド |
| `make test` | テスト実行 |
| `make fmt` | コードフォーマット |
| `make clippy` | リント（warnings = error） |
| `make check` | fmt + clippy + test を一括実行 |

## Project Structure

```
lifeai/
├── core/                 # Rust メインソースコード
├── .claude/
│   ├── skills/           # Claude Code スキル群
│   │   ├── daily-report/ # 対話型日報作成
│   │   ├── insight-report/ # 使用状況分析 & 改善
│   │   └── gen-skill/    # スキルスキャフォールド生成
│   └── agents/           # カスタムエージェント
├── .devcontainer/        # Dev Container 設定
├── .githooks/            # Git hooks (pre-commit, pre-push)
└── Makefile
```
