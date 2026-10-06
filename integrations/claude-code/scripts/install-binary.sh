#!/bin/sh
# Installs the ripwire-broker release this plugin pins (spec/plan/mod-plan.md, T2.2; DM-1).
#
#   sh install-binary.sh [--target TRIPLE] [--from DIR]   download, verify, install
#   sh install-binary.sh --prune                          remove the other installed versions
#
# The release is the tag on the first line of scripts/checksums.txt; each further line is
# `sha256  asset`. The asset is downloaded from the GitHub release (or copied from --from DIR),
# checked against its pinned SHA-256, and only then unpacked into
# ${CLAUDE_PLUGIN_DATA}/bin/<version>/ripwire-broker. Nothing is downloaded unless you run this:
# the SessionStart hook only checks.
set -eu

root=${CLAUDE_PLUGIN_ROOT:-$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)}
# Outside a hook (from a terminal, or the Bash tool) Claude Code does not export the plugin's
# variables; this is where it keeps the data of ripwire-broker@aquental.
data=${CLAUDE_PLUGIN_DATA:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}/plugins/data/ripwire-broker-aquental}
repo=https://github.com/aquental/ripwire-broker

die() {
    echo "install-binary: $*" >&2
    exit 1
}

target=
from=
prune=
while [ $# -gt 0 ]; do
    case $1 in
        --target) target=${2:?--target needs a value}; shift 2 ;;
        --from) from=${2:?--from needs a value}; shift 2 ;;
        --prune) prune=1; shift ;;
        *) die "unknown argument: $1" ;;
    esac
done

tag=$(sed -n '1p' "$root/scripts/checksums.txt")
version=${tag#v}
[ -n "$version" ] || die "no release pinned in $root/scripts/checksums.txt"

if [ -n "$prune" ]; then
    for dir in "$data"/bin/*/; do
        [ -d "$dir" ] || continue
        name=$(basename "$dir")
        [ "$name" = "$version" ] || rm -rf -- "${data:?}/bin/$name"
    done
    exit 0
fi

if [ -z "$target" ]; then
    case $(uname -s)-$(uname -m) in
        Darwin-arm64) target=aarch64-apple-darwin ;;
        Darwin-x86_64) target=x86_64-apple-darwin ;;
        Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
        Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-gnu ;;
        *) die "no release build for $(uname -s) $(uname -m); use cargo install --git $repo --tag $tag --features online" ;;
    esac
fi

asset=ripwire-broker-$tag-$target.tar.gz
want=$(sed -n "2,\$s/^\([0-9a-f]\{64\}\)  $asset\$/\1/p" "$root/scripts/checksums.txt")
[ -n "$want" ] || die "$tag pins no asset for $target; use cargo install --git $repo --tag $tag --features online"

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

mkdir -p "$data/bin"
work=$(mktemp -d "$data/bin/.install.XXXXXX")
trap 'rm -rf -- "$work"' EXIT INT TERM

if [ -n "$from" ]; then
    cp -- "$from/$asset" "$work/$asset"
elif command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$work/$asset" "$repo/releases/download/$tag/$asset"
elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$work/$asset" "$repo/releases/download/$tag/$asset"
else
    die "neither curl nor wget is installed"
fi

got=$(sha256 "$work/$asset")
[ "$got" = "$want" ] || die "SHA-256 mismatch for $asset: expected $want, got $got; nothing installed"

mkdir "$work/unpacked"
tar -xzf "$work/$asset" -C "$work/unpacked" ripwire-broker
chmod 755 "$work/unpacked/ripwire-broker"
mkdir -p "$data/bin/$version"
# Same file system: the binary appears whole or not at all.
mv -f "$work/unpacked/ripwire-broker" "$data/bin/$version/ripwire-broker"
echo "installed ripwire-broker $version at $data/bin/$version/ripwire-broker"
