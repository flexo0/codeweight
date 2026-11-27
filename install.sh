#!/bin/sh
# Installs codeweight's release binary on Linux or macOS:
#
#   curl -LsSf https://github.com/flexo0/codeweight/releases/latest/download/install.sh | sh
#
# CODEWEIGHT_VERSION picks the release (`0.2.0` or `v0.2.0`; the default: this script's own release, else the latest),
# CODEWEIGHT_INSTALL_DIR the folder (default `~/.local/bin`). The archive is checked against its `.sha256` before
# anything is installed. How it's released: docs/releasing.md.

set -eu

# The release this copy is attached to, set by release.yml; empty in the repo, where the latest is fetched.
release_tag=""

repo="flexo0/codeweight"

say() {
    printf 'codeweight: %s\n' "$*"
}

fail() {
    printf 'codeweight: error: %s\n' "$*" >&2
    exit 1
}

# Downloads a URL to a file (`-` for stdout), by curl or else wget.
fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl --proto '=https' --tlsv1.2 -fsSL -o "$2" "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$2" "$1"
    else
        fail "neither curl nor wget is installed"
    fi
}

# The release's target triple for this OS and CPU.
detect_target() {
    os=$(uname -s)
    cpu=$(uname -m)
    case "$os" in
        Linux)
            case "$cpu" in
                x86_64 | amd64) echo x86_64-unknown-linux-musl ;;
                *) fail "no release binary for Linux on $cpu yet; build it with \`cargo install codeweight\`" ;;
            esac
            ;;
        Darwin)
            # A shell under Rosetta says x86_64 on Apple silicon: the native binary runs there all the same.
            if [ "$cpu" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then
                cpu=arm64
            fi
            case "$cpu" in
                arm64 | aarch64) echo aarch64-apple-darwin ;;
                x86_64) echo x86_64-apple-darwin ;;
                *) fail "no release binary for macOS on $cpu" ;;
            esac
            ;;
        MINGW* | MSYS* | CYGWIN*)
            fail "on Windows, run install.ps1: powershell -c \"irm https://github.com/$repo/releases/latest/download/install.ps1 | iex\""
            ;;
        *) fail "no release binary for $os; build it with \`cargo install codeweight\`" ;;
    esac
}

# The tag to install: CODEWEIGHT_VERSION's, else this script's release, else the latest release's.
pick_tag() {
    if [ -n "${CODEWEIGHT_VERSION:-}" ]; then
        case "$CODEWEIGHT_VERSION" in
            v*) echo "$CODEWEIGHT_VERSION" ;;
            *) echo "v$CODEWEIGHT_VERSION" ;;
        esac
    elif [ -n "$release_tag" ]; then
        echo "$release_tag"
    else
        tag=$(fetch "https://api.github.com/repos/$repo/releases/latest" - |
            sed -n 's/^ *"tag_name": *"\([^"]*\)".*/\1/p')
        [ -n "$tag" ] || fail "couldn't find the latest release of $repo"
        echo "$tag"
    fi
}

# The SHA-256 of a file, by sha256sum or else shasum.
sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d ' ' -f 1
    else
        fail "neither sha256sum nor shasum is installed, so the download can't be checked"
    fi
}

main() {
    target=$(detect_target)
    tag=$(pick_tag)
    dir="${CODEWEIGHT_INSTALL_DIR:-$HOME/.local/bin}"
    name="codeweight-$tag-$target"
    url="https://github.com/$repo/releases/download/$tag/$name.tar.gz"

    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT

    say "downloading $name.tar.gz"
    fetch "$url" "$tmp/$name.tar.gz" || fail "couldn't download $url"
    fetch "$url.sha256" "$tmp/$name.tar.gz.sha256" || fail "couldn't download $url.sha256"
    expected=$(cut -d ' ' -f 1 <"$tmp/$name.tar.gz.sha256")
    actual=$(sha256 "$tmp/$name.tar.gz")
    [ "$expected" = "$actual" ] || fail "$name.tar.gz's SHA-256 is $actual, not the release's $expected"

    tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
    mkdir -p "$dir"
    # Into a temporary name first, so a running codeweight or an interrupted copy never leaves half a binary.
    cp "$tmp/$name/codeweight" "$dir/.codeweight.tmp"
    chmod 755 "$dir/.codeweight.tmp"
    mv -f "$dir/.codeweight.tmp" "$dir/codeweight"
    say "installed $tag to $dir/codeweight"

    case ":$PATH:" in
        *":$dir:"*) ;;
        *)
            say "$dir isn't on your PATH; add it, e.g. to ~/.profile or ~/.zshrc:"
            printf '\n    export PATH="%s:%s"\n\n' "$dir" "\$PATH"
            ;;
    esac
}

# The whole script is read before main runs, so a download cut short by `curl | sh` runs nothing.
main "$@"
