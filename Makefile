.PHONY: ota install-dashboard check-firmware build-firmware check-dashboard-format check-dashboard-lint check-dashboard-test check-dashboard build-dashboard check-daemon build-daemon

ifeq ($(OS),Windows_NT)
PYTHON ?= python
else
PYTHON ?= python3
endif

export OTA_IP

ota:
	@python3 scripts/ota_upload.py

install-dashboard:
	cd dashboard && bun install --frozen-lockfile

check-firmware:
	./scripts/test_host.sh

build-firmware:
	pio run

check-dashboard-format:
	cd dashboard && bun run format:check

check-dashboard-lint:
	cd dashboard && bun run lint

check-dashboard-test:
	cd dashboard && bun run test

check-dashboard: check-dashboard-format check-dashboard-lint check-dashboard-test

build-dashboard:
	cd dashboard && bun run build

check-daemon:
	cargo fmt --manifest-path daemon/Cargo.toml --all -- --check
	cargo clippy --manifest-path daemon/Cargo.toml --locked --all-targets -- -D warnings
	cargo test --manifest-path daemon/Cargo.toml --locked --all-targets
	$(PYTHON) -m py_compile daemon/test_https_daemon.py
	$(PYTHON) -c "import json; json.load(open('daemon/resterm/resterm.env.example.json'))"
	$(PYTHON) daemon/test_https_daemon.py --help

build-daemon:
	cargo build --manifest-path daemon/Cargo.toml --release --locked --target x86_64-pc-windows-msvc
