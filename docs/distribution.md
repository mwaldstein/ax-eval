# Distribution

**Status: Implemented**

## Installation Methods

### Primary: Pre-built Binaries (GitHub Releases)

Tagged releases publish pre-built binaries for all targets via the `release.yml` workflow:

| Platform | Architecture | Runner |
|----------|-------------|--------|
| macOS    | aarch64 | `macos-14` |
| Linux    | x86_64 | `ubuntu-latest` |
| Linux    | aarch64 | `ubuntu-24.04-arm` |
| Windows  | x86_64 | `windows-latest` |

Binary naming: `ax-eval-<version>-<target>.tar.gz` (`.zip` for Windows).

### Quick Install Scripts

**Unix (macOS/Linux):**

```bash
curl -fsSL https://raw.githubusercontent.com/mwaldstein/ax-eval/master/scripts/install.sh | sh
```

The installer:
- Detects platform and architecture
- Downloads the appropriate binary from GitHub releases
- Ignores prereleases by default (set `AX_EVAL_INCLUDE_PRERELEASES=1` to include)
- Installs to `~/.local/bin` (override with `INSTALL_DIR`)
- Verifies SHA-256 checksums
- Prints PATH setup guidance if needed

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/mwaldstein/ax-eval/master/scripts/install.ps1 | iex
```

Same behavior as the Unix installer: platform detection, checksum verification, prerelease filtering.

### Environment Variables (Install Scripts)

| Variable | Default | Description |
|----------|---------|-------------|
| `AX_EVAL_REPO` | `mwaldstein/ax-eval` | GitHub repo for releases |
| `INSTALL_DIR` | `~/.local/bin` | Installation directory |
| `AX_EVAL_VERSION` | `latest` | Version to install (or `latest`) |
| `AX_EVAL_INCLUDE_PRERELEASES` | `0` | Set `1`/`true` to consider prereleases |

### Cargo Install

```bash
cargo install ax-eval
```

Published to crates.io. The `release.yml` workflow publishes on tag push. CI runs `cargo publish --dry-run` on every push to catch metadata issues early.

---

## Release Automation

### CI (`ci.yml`)

Runs on push to `master` and on pull requests:

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all-targets`
4. `cargo build --release --locked`

### Release (`release.yml`)

Triggered by pushing a `v*` tag. Three jobs:

1. **Validate** — checks tag version matches `Cargo.toml` version, detects prerelease (version contains `-`).

2. **Build** — matrix across all 4 targets, builds release binaries, packages as `.tar.gz` (Unix) or `.zip` (Windows), uploads as artifacts.

3. **Publish GitHub release** — downloads all build artifacts, generates `SHA256SUMS`, creates GitHub release with `softprops/action-gh-release@v3`. Prerelease tags are marked accordingly.

4. **Publish to crates.io** — runs `cargo publish --locked`. Requires `CARGO_REGISTRY_TOKEN` secret.

### Targets

| Target | Archive |
|--------|---------|
| `aarch64-apple-darwin` | `.tar.gz` |
| `x86_64-unknown-linux-gnu` | `.tar.gz` |
| `aarch64-unknown-linux-gnu` | `.tar.gz` |
| `x86_64-pc-windows-msvc` | `.zip` |

### Versioning

- Semver (`MAJOR.MINOR.PATCH`)
- Git tags: `v1.2.3`
- Prerelease tags: `v1.2.3-beta.1` (version contains `-`)
- Tag version must match `Cargo.toml` version (enforced by `release.yml`)

### Checksums

Every release includes a `SHA256SUMS` file. Install scripts verify checksums before executing binaries.

---

## Repository Structure

```
scripts/
  install.sh       # Unix installer (macOS/Linux)
  install.ps1      # Windows installer (PowerShell)
.github/
  workflows/
    ci.yml         # CI: lint, test, build on push/PR
    release.yml    # Release: build + publish on tag
```
