fmt: format ## Check formatting of the code (alias for 'format')
format: ## Check formatting of the code
	cargo fmt --all -- --check

clippy: lint ## Check quality of the code (alias for 'lint')
lint: ## Check quality of the code
	cargo clippy --workspace --all-features --all-targets -- --warn clippy::cargo --allow clippy::multiple_crate_versions --deny warnings

test: ## Launch all tests
	# Run all the tests of `transit_model` in the entire repository.

	# First activating all features (including `xmllint`)
	cargo test --workspace --all-features
	# Then without features
	cargo test --workspace

help: ## Print this help message
	@grep -E '^[a-zA-Z_-]+:.*## .*$$' $(CURDIR)/$(firstword $(MAKEFILE_LIST)) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

.PHONY: fmt format clippy lint test help
.DEFAULT_GOAL := help
