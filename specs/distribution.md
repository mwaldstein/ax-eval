# Distribution

**Status: Early Draft**

## Goals

- Provide multiple installation methods to meet users where they are.
- Minimize friction: one command should get a working `llm-tool-test` binary.
- Support all major developer platforms (macOS, Linux, Windows).
- Enable automated releases with pre-built binaries.

## Non-goals

- Distribution of plugins or extensions (out of scope for this spec).
- Package manager submission/maintenance automation (manual for now).

## Installation Methods

### Primary: Pre-built Binaries (GitHub Releases)

Each tagged release publishes pre-built binaries for:

| Platform | Architecture |
|----------|--------------|
| macOS    | aarch64 |
| Linux    | x86_64, aarch64 |
| Windows  | x86_64 |

Binary naming convention: `llm-tool-test-<version>-<target>.tar.gz` (or `.zip` for Windows).

### Quick Install Scripts

**Unix (macOS/Linux):**

```bash
curl -fsSL https://raw.githubusercontent.com/mwaldstein/llm-tool-test/master/scripts/install.sh | sh
```

The installer should:
- Detect platform and architecture
- Download the appropriate binary from GitHub releases, ignoring prereleases by default
- Install to `~/.local/bin` (or `/usr/local/bin` with `sudo`)
- Verify checksums
- Provide PATH setup guidance if needed

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/mwaldstein/llm-tool-test/master/scripts/install.ps1 | iex
```

### Cargo Install

```bash
cargo install llm-tool-test
```

Requires publishing to crates.io. This is the canonical "from source" method for Rust users.

### Homebrew (macOS/Linux)

```bash
brew tap mwaldstein/llm-tool-test
brew install llm-tool-test
```

Requires:
- A Homebrew tap repository (`homebrew-llm-tool-test`)
- Formula that downloads pre-built binaries or builds from source

### Package Managers (Future)

Candidates for future support:

| Manager | Platform | Priority |
|---------|----------|----------|
| AUR     | Arch Linux | Medium |
| Nix     | NixOS/macOS | Medium |
| winget  | Windows | Low |
| Scoop   | Windows | Low |
| deb/rpm | Debian/RHEL | Low |

## Release Automation

### GitHub Actions Workflow

On tagged releases (`v*`), automation should:

1. Build binaries for all target platforms (cross-compilation or matrix)
2. Generate checksums (`SHA256SUMS`)
3. Create GitHub release with binaries and checksums attached
4. Optionally publish to crates.io

### Targets

Use Rust cross-compilation targets:

- `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

### Versioning

- Follow semver (`MAJOR.MINOR.PATCH`)
- Git tags: `v1.2.3`
- `llm-tool-test --version` output must match the release tag

## Verification

### Checksums

Every release includes a `SHA256SUMS` file. Install scripts should verify checksums before executing binaries.

### Signatures (Future)

Consider GPG or sigstore signing for releases.

## Repository Structure

```
scripts/
  install.sh       # Unix installer
  install.ps1      # Windows installer
.github/
  workflows/
    release.yml    # Release automation
```

## Validation

This spec is considered implemented when:

- `cargo install llm-tool-test` works from crates.io
- GitHub releases include binaries for all listed platforms
- Install scripts successfully install on macOS, Linux, and Windows
- `brew install` works via tap

## References

- [beads INSTALLING.md](https://github.com/steveyegge/beads/blob/main/docs/INSTALLING.md) — inspiration for multi-method distribution
