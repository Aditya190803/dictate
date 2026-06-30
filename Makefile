PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
CARGO ?= cargo
FEATURES ?=

RELEASE_DIR := target/release
BINARIES := dictate

.PHONY: all build install test clean

all: build

build:
	$(CARGO) build --release $(if $(FEATURES),--features $(FEATURES),)

install: build
	install -d "$(BINDIR)"
	for bin in $(BINARIES); do \
		install -m 0755 "$(RELEASE_DIR)/$$bin" "$(BINDIR)/$$bin.new"; \
		mv "$(BINDIR)/$$bin.new" "$(BINDIR)/$$bin"; \
	done
	"$(BINDIR)/dictate" --version
	@echo ""
	@echo "Next: dictate setup   (config + shortcut snippets for SUPER,R and SUPER+SHIFT+R)"

test:
	$(CARGO) test

clean:
	$(CARGO) clean