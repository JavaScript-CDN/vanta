#!/bin/bash
# Vanta engine build helpers. Structure adapted from
# ungoogled-chromium-portablelinux (BSD-3-Clause), see THIRD_PARTY_NOTICES.md.
set -euo pipefail

repo_root() { cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd; }

setup_paths() {
    _root="$(repo_root)"
    _build_dir="${_root}/build"
    _dl_cache="${_build_dir}/download_cache"
    _src_dir="${_build_dir}/src"
    _out_dir="${_src_dir}/out/Default"
    _version="$(tr -d '[:space:]' < "${_root}/engine/chromium_version.txt")"
    _sha256="$(tr -d '[:space:]' < "${_root}/engine/chromium_sha256.txt")"
    mkdir -p "${_dl_cache}"
}

# Download the Chromium release tarball and verify it against the SHA-256
# pinned in this repo (never trust a hash fetched from the same server).
fetch_sources() {
    if [ -f "${_src_dir}/.downloaded.stamp" ]; then
        echo "sources already unpacked"; return 0
    fi
    local name="chromium-${_version}-linux.tar.xz"
    local url="https://github.com/chromium-linux-tarballs/chromium-tarballs/releases/download/${_version}/${name}"
    local file="${_dl_cache}/${name}"
    if [ ! -f "${file}" ]; then
        curl --fail --location --retry 5 --retry-delay 10 --proto '=https' --tlsv1.2 -o "${file}.part" "${url}"
        mv "${file}.part" "${file}"
    fi
    echo "${_sha256}  ${file}" | sha256sum -c -
    mkdir -p "${_src_dir}"
    tar -xJf "${file}" -C "${_src_dir}" --strip-components=1 --no-same-owner
    rm -f "${file}"
    touch "${_src_dir}/.downloaded.stamp"
}

# Apply Vanta patches listed in patches/series, in order. Any failure stops the build.
apply_patches() {
    [ -f "${_src_dir}/.patched.stamp" ] && return 0
    local p
    while IFS= read -r p || [ -n "$p" ]; do
        p="${p%%#*}"; p="$(echo "$p" | xargs)"
        [ -z "$p" ] && continue
        echo ">> applying patches/${p}"
        patch -p1 --forward --fuzz=0 --no-backup-if-mismatch -d "${_src_dir}" < "${_root}/patches/${p}"
    done < "${_root}/patches/series"
    touch "${_src_dir}/.patched.stamp"
}

write_gn_args() {
    mkdir -p "${_out_dir}"
    { cat "${_root}/engine/args.gn"; echo 'target_cpu = "x64"'; } > "${_out_dir}/args.gn"
    cat "${_out_dir}/args.gn"
}

link_if_missing() { # $1 = target binary, $2 = link path
    if [ ! -e "$2" ]; then mkdir -p "$(dirname "$2")"; ln -sf "$1" "$2"; fi
}

setup_toolchain() {
    cd "${_src_dir}"
    python3 tools/rust/update_rust.py
    python3 tools/clang/scripts/update.py
    python3 build/linux/sysroot_scripts/install-sysroot.py --arch=x64

    link_if_missing "$(command -v node)"  third_party/node/linux/node-linux-x64/bin/node
    link_if_missing "$(command -v gperf)" third_party/gperf/cipd/bin/gperf
    link_if_missing "$(command -v go)"    third_party/dawn/tools/golang/linux-amd64/bin/go
    link_if_missing "$(command -v clang-format)" buildtools/linux64-format/clang-format
    link_if_missing "$(command -v tsc)"   third_party/typescript/linux-amd64/src/lib/tsc
    if [ ! -e third_party/devtools-frontend/src/node_modules/esbuild/bin/esbuild ]; then
        rm -rf third_party/devtools-frontend/src/node_modules/esbuild
        ln -s "$(npm root -g)/esbuild" third_party/devtools-frontend/src/node_modules/esbuild
    fi
}

gn_gen() {
    cd "${_src_dir}"
    local llvm="${_src_dir}/third_party/llvm-build/Release+Asserts/bin"
    CC="${llvm}/clang" CXX="${llvm}/clang++" AR="${llvm}/llvm-ar" \
        python3 tools/gn/bootstrap/bootstrap.py -o out/Default/gn --skip-generate-buildfiles
    ./out/Default/gn gen out/Default
    ./out/Default/gn args out/Default --list --short --overrides-only
}
