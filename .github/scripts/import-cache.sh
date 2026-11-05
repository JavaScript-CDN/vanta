#!/bin/bash
set -euxo pipefail
_base="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
zstd -d -c "$_base/.github/cache/build-tree.tar.zst" | tar -xf - -C "$_base"
rm -f "$_base/.github/cache/build-tree.tar.zst"
df -h /
