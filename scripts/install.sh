#!/usr/bin/env sh
set -eu

repo="${LLM_TOOL_TEST_REPO:-mwaldstein/llm-tool-test}"
install_dir="${INSTALL_DIR:-$HOME/.local/bin}"
version="${LLM_TOOL_TEST_VERSION:-latest}"
include_prereleases="${LLM_TOOL_TEST_INCLUDE_PRERELEASES:-0}"
bin_name="llm-tool-test"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'error: required command not found: %s\n' "$1" >&2
    exit 1
  fi
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin) os_part="apple-darwin" ;;
    Linux) os_part="unknown-linux-gnu" ;;
    *)
      printf 'error: unsupported OS: %s\n' "$os" >&2
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64 | amd64) arch_part="x86_64" ;;
    arm64 | aarch64) arch_part="aarch64" ;;
    *)
      printf 'error: unsupported architecture: %s\n' "$arch" >&2
      exit 1
      ;;
  esac

  printf '%s-%s' "$arch_part" "$os_part"
}

download() {
  url="$1"
  output="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$output"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$url" -O "$output"
  else
    printf 'error: required command not found: curl or wget\n' >&2
    exit 1
  fi
}

resolve_version() {
  if [ "$version" != "latest" ]; then
    printf '%s' "${version#v}"
    return
  fi

  need_cmd sed
  tmp_json="$tmp_dir/latest.json"
  if [ "$include_prereleases" = "1" ] || [ "$include_prereleases" = "true" ]; then
    download "https://api.github.com/repos/$repo/releases" "$tmp_json"
  else
    # GitHub's latest endpoint ignores prereleases.
    download "https://api.github.com/repos/$repo/releases/latest" "$tmp_json"
  fi
  tag="$(sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' "$tmp_json" | sed -n '1p')"
  if [ -z "$tag" ]; then
    printf 'error: unable to resolve latest release for %s\n' "$repo" >&2
    exit 1
  fi
  printf '%s' "${tag#v}"
}

verify_checksum() {
  sums_file="$1"
  asset="$2"

  need_cmd grep
  line="$(grep "  $asset\$" "$sums_file" || true)"
  expected="${line%% *}"
  if [ -z "$expected" ]; then
    printf 'error: checksum for %s not found\n' "$asset" >&2
    exit 1
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$asset" | sed -n 's/ .*//p')"
  elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "$asset" | sed -n 's/ .*//p')"
  else
    printf 'error: required command not found: sha256sum or shasum\n' >&2
    exit 1
  fi

  if [ "$expected" != "$actual" ]; then
    printf 'error: checksum mismatch for %s\n' "$asset" >&2
    exit 1
  fi
}

target="$(detect_target)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT INT TERM

need_cmd tar

resolved_version="$(resolve_version)"
asset="${bin_name}-${resolved_version}-${target}.tar.gz"
base_url="https://github.com/${repo}/releases/download/v${resolved_version}"

cd "$tmp_dir"
download "$base_url/$asset" "$asset"
download "$base_url/SHA256SUMS" "SHA256SUMS"
verify_checksum "SHA256SUMS" "$asset"

tar -xzf "$asset"
mkdir -p "$install_dir"
cp "$bin_name" "$install_dir/$bin_name"
chmod 755 "$install_dir/$bin_name"

printf 'Installed %s %s to %s/%s\n' "$bin_name" "$resolved_version" "$install_dir" "$bin_name"
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) printf 'Add %s to PATH to run %s from any shell.\n' "$install_dir" "$bin_name" ;;
esac
