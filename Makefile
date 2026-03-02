# Default directories
DOCS_DIR ?= docs
BIN_DIR ?= zig-out/bin

# Parse positional arguments for build_docs (everything after target)
ifeq (build_docs,$(firstword $(MAKECMDGOALS)))
  ifneq ($(word 2,$(MAKECMDGOALS)),)
    DOCS_DIR := $(word 2,$(MAKECMDGOALS))
    $(eval $(DOCS_DIR):;@:)
  endif
endif

# Parse positional arguments for build (everything after target)
ifeq (build,$(firstword $(MAKECMDGOALS)))
  ifneq ($(word 2,$(MAKECMDGOALS)),)
    BIN_DIR := $(word 2,$(MAKECMDGOALS))
    $(eval $(BIN_DIR):;@:)
  endif
endif

# Parse positional arguments for run (everything after target goes to CLI)
ifeq (run,$(firstword $(MAKECMDGOALS)))
  RUN_ARGS := $(wordlist 2,$(words $(MAKECMDGOALS)),$(MAKECMDGOALS))
  $(eval $(RUN_ARGS):;@:)
endif

.PHONY: help all install_base install_deps build_docs build test run build_docker run_docker

.DEFAULT_GOAL := help
all: help

help:
	@echo "Available tasks:"
	@echo "  install_base     Install language runtime (Zig) and native deps"
	@echo "  install_deps     Install local dependencies (none required for Zig)"
	@echo "  build_docs [dir] Build API docs. Optional alternative dir (default: docs)"
	@echo "  build [dir]      Build CLI binary. Optional alternative dir (default: zig-out/bin)"
	@echo "  test             Run tests locally"
	@echo "  run [args...]    Build and run the CLI. Appends any args to the CLI."
	@echo "  build_docker     Build Alpine and Debian Docker images"
	@echo "  run_docker       Run and test Docker images locally"

install_base:
	@echo "Installing base dependencies..."
	@if [ -f /etc/debian_version ]; then 		sudo apt-get update && sudo apt-get install -y wget xz-utils; 	elif [ -f /etc/alpine-release ]; then 		sudo apk add wget tar xz; 	elif [ -f /etc/redhat-release ]; then 		sudo dnf install -y wget xz; 	elif [ "$$(uname -s)" = "Darwin" ]; then 		brew install wget; 	elif [ "$$(uname -s)" = "FreeBSD" ]; then 		sudo pkg install -y wget; 	fi
	@echo "Downloading Zig 0.13.0 for Linux..."
	@wget -qO- https://ziglang.org/download/0.13.0/zig-linux-$$(uname -m)-0.13.0.tar.xz | tar -xJ || echo "Please install Zig manually from ziglang.org"

install_deps:
	@echo "No external packages to install. Zig manages deps internally via build.zig.zon."

build_docs:
	zig build docs --prefix $(DOCS_DIR)

build:
	zig build -Doptimize=ReleaseSafe --prefix $(BIN_DIR)

test:
	zig build test

run: build
	$(BIN_DIR)/cdd-ctl $(RUN_ARGS)

build_docker:
	docker build -t cdd-ctl-alpine -f alpine.Dockerfile .
	docker build -t cdd-ctl-debian -f debian.Dockerfile .

run_docker:
	@echo "Testing Alpine Image..."
	docker run -d -p 8080:8080 --name cdd-ctl-alpine-test cdd-ctl-alpine
	sleep 2
	curl -s http://localhost:8080
	docker stop cdd-ctl-alpine-test
	docker rm cdd-ctl-alpine-test
	@echo "Testing Debian Image..."
	docker run -d -p 8081:8080 --name cdd-ctl-debian-test cdd-ctl-debian
	sleep 2
	curl -s http://localhost:8081
	docker stop cdd-ctl-debian-test
	docker rm cdd-ctl-debian-test
	docker rmi cdd-ctl-alpine cdd-ctl-debian
