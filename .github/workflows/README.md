# GitHub Actions workflows

- `ci.yml` runs on every push to `main` / `develop` and on every pull request. It
  builds and tests the Rust workspace on Ubuntu and Windows (`cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`) and runs
  the web UI check path (`npm ci && npm run check` in `web/`), including typecheck,
  the button-map p95 guard, and the production build.

## Running locally

```sh
# Rust
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Web
cd web && npm ci && npm run check
```

On Linux you may need `sudo apt-get install -y libudev-dev` so `hidapi` compiles.
