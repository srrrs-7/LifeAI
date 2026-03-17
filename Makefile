.PHONY: build test fmt clippy check hooks init-firewall

build:
	cargo build

test:
	cargo test

fmt:
	cargo fmt

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

check: fmt clippy test

hooks:
	git config core.hooksPath "$$(pwd)/.githooks"
	chmod +x .githooks/pre-commit .githooks/pre-push
	@echo "Git hooks installed to $$(pwd)/.githooks"

init-firewall:
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
