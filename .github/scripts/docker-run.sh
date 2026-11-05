#!/bin/bash
set -euo pipefail
_base="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
_env=()
for v in _prepare_only _gha_final _stage_timeout GITHUB_OUTPUT; do
    [ -n "${!v:-}" ] && _env+=(-e "$v")
done
docker run --rm -i -u "$(id -u):$(id -g)" \
    -v "${_base}:/repo" -v "${GITHUB_OUTPUT}:${GITHUB_OUTPUT}" \
    "${_env[@]}" vanta-builder:local bash /repo/.github/scripts/ci-build.sh
