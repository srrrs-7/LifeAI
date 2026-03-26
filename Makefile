# ============================================================
# LifeAI Makefile
# ============================================================
.PHONY: help build build-release test fmt clippy check clean \
        hooks init-firewall \
        build-otel-cc test-otel-cc \
        metrics open-grafana open-prometheus

# ------------------------------------------------------------
# Default
# ------------------------------------------------------------
help: ## ヘルプ表示
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-24s\033[0m %s\n", $$1, $$2}'

# ------------------------------------------------------------
# Rust — ビルド・テスト・リント
# ------------------------------------------------------------
build: ## cargo build（デバッグ）
	cargo build

build-release: ## cargo build --release
	cargo build --release

test: ## テスト実行
	cargo test

fmt: ## cargo fmt
	cargo fmt

fmt-check: ## フォーマットチェック（CI 用）
	cargo fmt -- --check

clippy: ## リント（warnings = error）
	cargo clippy --all-targets --all-features -- -D warnings

check: fmt clippy test ## fmt + clippy + test を一括実行

clean: ## target/ を削除
	cargo clean

# ------------------------------------------------------------
# Rust — クレート個別
# ------------------------------------------------------------
build-otel-cc: ## otel-cc のみビルド
	cargo build -p otel-cc

test-otel-cc: ## otel-cc のみテスト
	cargo test -p otel-cc

# ------------------------------------------------------------
# 動作確認・ユーティリティ
# ------------------------------------------------------------
metrics: ## /metrics エンドポイントを確認（otel-cc コンテナ経由）
	@docker compose -f .devcontainer/compose.yaml exec otel-cc curl -sf http://localhost:9091/metrics | head -40 || echo "otel-cc is not running"

open-grafana: ## Grafana をブラウザで開く
	@echo "http://localhost:3000"

open-prometheus: ## Prometheus をブラウザで開く
	@echo "http://localhost:9090"

# ------------------------------------------------------------
# Git Hooks
# ------------------------------------------------------------
hooks: ## Git hooks インストール
	git config core.hooksPath "$$(pwd)/.githooks"
	chmod +x .githooks/pre-commit .githooks/pre-push
	@echo "Git hooks installed to $$(pwd)/.githooks"

# ------------------------------------------------------------
# Dev Container
# ------------------------------------------------------------
init-firewall: ## ファイアウォール初期化（要 root）
	@if [ "$$(id -u)" -ne 0 ]; then \
		if command -v sudo >/dev/null 2>&1; then \
			sudo ./.devcontainer/init-firewall.sh; \
		else \
			echo "ERROR: init-firewall requires root; please run 'sudo make init-firewall'"; \
			exit 1; \
		fi; \
	else \
		./.devcontainer/init-firewall.sh; \
	fi
