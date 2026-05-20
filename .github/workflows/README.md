# GitHub Actions workflows

- `ci.yml` runs on every push to `main` / `develop` and on every pull request. It
  builds and tests the Rust workspace on Ubuntu and Windows (`cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`) and builds
  the web UI (`npm ci && npm run build` in `web/`).

## Running locally

```sh
# Rust
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Web
cd web && npm ci && npm run build
```

On Linux you may need `sudo apt-get install -y libudev-dev` so `hidapi` compiles.
