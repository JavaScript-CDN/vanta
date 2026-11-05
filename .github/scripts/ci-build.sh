#!/bin/bash
# Runs INSIDE the build container. Either prepares the tree or runs one
# time-boxed ninja stage; the tree is carried to the next job as an artifact.
set -euxo pipefail
. /repo/scripts/shared.sh
setup_paths

if [ "${_prepare_only:-false}" = "true" ]; then
    fetch_sources
    apply_patches
    write_gn_args
    setup_toolchain
    gn_gen
    exit 0
fi

cd "${_src_dir}"
_stage_timeout="${_stage_timeout:-17400}"   # 4h50m, leaves time for cache export
set +e
timeout -k 5m -s INT "${_stage_timeout}s" ninja -C out/Default chrome chromedriver
rc=$?
set -e

if [ "$rc" -eq 0 ] && [ -x "${_out_dir}/chrome" ]; then
    echo "status=completed" >> "$GITHUB_OUTPUT"
    exit 0
fi
if [ "$rc" -eq 124 ] || [ "$rc" -eq 130 ]; then
    if [ "${_gha_final:-false}" = "true" ]; then
        echo "::error::last stage ran out of time"; exit 1
    fi
    echo "stage time box reached; continuing in next job"
    echo "status=running" >> "$GITHUB_OUTPUT"
    exit 0
fi
exit "$rc"
