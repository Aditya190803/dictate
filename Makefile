PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
CARGO ?= cargo
FEATURES ?= words-ui

RELEASE_DIR := target/release
BINARIES := dictate

# Feature matrix: default (online providers only), local (whisper-rs),
# words-ui (GTK4 dictionary GUI). local and words-ui are Linux-only
# (see docs/windows.md); default builds everywhere, including Windows.
FEATURE_MATRIX ?= default local words-ui

.PHONY: all build install test clean fmt clippy check check-features

all: build

build:
	$(CARGO) build --release $(if $(FEATURES),--features $(FEATURES),)

# Unix install (install(1) + atomic mv). On Windows there is no
# install(1): use `copy` per docs/windows.md instead:
#   cargo build --release
#   copy target\release\dictate.exe %USERPROFILE%\bin\dictate.exe
install: build
	install -d "$(BINDIR)"
	for bin in $(BINARIES); do \
		install -m 0755 "$(RELEASE_DIR)/$$bin" "$(BINDIR)/$$bin.new"; \
		mv "$(BINDIR)/$$bin.new" "$(BINDIR)/$$bin"; \
	done
	"$(BINDIR)/dictate" --version
	@echo ""
	@echo "Next: dictate setup   (config + shortcut snippets for SUPER,R and SUPER+SHIFT+R)"

fmt:
	$(CARGO) fmt --all

clippy:
	$(CARGO) clippy --all-targets -- -D warnings

# Default-feature check (mirrors CI's Build job).
check:
	$(CARGO) check --all-targets

# Feature-matrix coverage without paying for three full builds:
# `cargo check` type-checks each feature set (default, local, words-ui).
# Full `cargo test --features words-ui,local` also works but links GTK4 +
# whisper, so it is CI-only (Linux) and local opt-in, not part of `test`.
check-features:
	for f in $(FEATURE_MATRIX); do \
		if [ "$$f" = default ]; then \
			$(CARGO) check --all-targets; \
		else \
			$(CARGO) check --all-targets --features $$f; \
		fi; \
	done

test:
	$(CARGO) test

clean:
	$(CARGO) clean
