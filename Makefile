PROJ_VERSION = 6.3.0
install_proj: ## Install PROJ and clang (requirements to use proj crate)
	sudo apt update
	sudo apt install -y clang

	# Needed only for proj install
	sudo apt install -y wget build-essential pkg-config sqlite3 libsqlite3-dev

	# remove PROJ system version from packages
	sudo apt remove libproj-dev

	wget https://github.com/OSGeo/proj.4/releases/download/$(PROJ_VERSION)/proj-$(PROJ_VERSION).tar.gz
	tar -xzvf proj-$(PROJ_VERSION).tar.gz
	pushd proj-$(PROJ_VERSION)
	./configure --prefix=/usr && make
	sudo make install
	popd
	rm -rf proj-$(PROJ_VERSION) proj-$(PROJ_VERSION).tar.gz

fmt: format ## Check formatting of the code (alias for 'format')
format: ## Check formatting of the code
	cargo fmt --all -- --check

clippy: lint ## Check quality of the code (alias for 'lint')
lint: ## Check quality of the code
	cargo clippy --workspace --all-features --all-targets -- --warn clippy::cargo --allow clippy::multiple_crate_versions --deny warnings

test: ## Launch all tests
	# Run all the tests of `transit_model` in the entire repository.

	# First activating all features (including `xmllint`)
	cargo test --workspace --all-features --all-targets  # `--all-targets` but no doctests
	cargo test --workspace --all-features --doc          # doctests only
	# Then without features
	cargo test --workspace --all-targets                 # `--all-targets` but no doctests
	cargo test --workspace --doc                         # doctests only

help: ## Print this help message
	@grep -E '^[a-zA-Z_-]+:.*## .*$$' $(CURDIR)/$(firstword $(MAKEFILE_LIST)) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

.PHONY: install_proj fmt format clippy lint test help
.DEFAULT_GOAL := help
