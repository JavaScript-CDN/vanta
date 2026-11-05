#!/bin/bash
set -euxo pipefail
_base="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$_base"
[ -d build/src ] || { echo "no build tree"; exit 1; }
rm -rf build/download_cache
mkdir -p .github/cache
tar -cf - build | zstd -f -T0 -3 -o .github/cache/build-tree.tar.zst
ls -lh .github/cache
