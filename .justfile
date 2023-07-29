# just manual: https://github.com/casey/just/#readme

_default:
    @just --list

# Format source code
fmt:
    cargo fmt --all

# Run clippy for various feature combinations: default, no default, all
clippy:
    cargo clippy --locked --workspace --no-deps --all-targets -- -D warnings
    cargo clippy --locked --workspace --no-deps --no-default-features --all-targets -- -D warnings
    cargo clippy --locked --workspace --no-deps --all-features --all-targets -- -D warnings

# Fix lint warnings
fix:
    cargo fix --workspace --all-features --all-targets
    cargo clippy --workspace --all-features --all-targets --fix

# Run unit tests for various feature combinations: default, no default, all
test:
    cargo test --locked --workspace -- --nocapture --include-ignored
    cargo test --locked --workspace --no-default-features -- --nocapture --include-ignored
    cargo test --locked --workspace --all-features -- --nocapture --include-ignored

# Set up (and update) tooling
setup:
    # Ignore rustup failures, because not everyone might use it
    rustup self update || true
    # cargo-edit is needed for `cargo upgrade`
    cargo install \
        just \
        cargo-edit \
        cargo-hack
    pip install -U pre-commit
    #pre-commit install --hook-type commit-msg --hook-type pre-commit

# Upgrade (and update) dependencies
upgrade: setup
    pre-commit autoupdate
    cargo upgrade
    cargo update

# Run pre-commit hooks
pre-commit:
    pre-commit run --all-files

# Check all lib/bin projects individually with selected features (takes a long time)
check-crates:
    cargo hack --feature-powerset check --locked --all-targets -p msr-core
    RUSTFLAGS="--cfg loom" cargo hack --feature-powerset check --locked --all-targets -p msr-core
    cargo hack --feature-powerset check --locked --all-targets -p msr-plugin
    cargo hack --feature-powerset check --locked --all-targets -p msr-plugin-csv-event-journal
    cargo hack --feature-powerset check --locked --all-targets -p msr-plugin-csv-register-recorder
