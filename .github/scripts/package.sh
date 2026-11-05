#!/bin/bash
# Collect the runtime files of the engine into a tarball with a SHA-256 sum.
set -euxo pipefail
_base="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
_out="$_base/build/src/out/Default"
_ver="$(tr -d '[:space:]' < "$_base/engine/chromium_version.txt")"
_name="vanta-engine-${_ver}-linux-x64"
_stage="$_base/dist/$_name"
mkdir -p "$_stage"
for f in chrome chrome_crashpad_handler chromedriver chrome_100_percent.pak chrome_200_percent.pak \
         resources.pak icudtl.dat v8_context_snapshot.bin snapshot_blob.bin libEGL.so libGLESv2.so \
         libvk_swiftshader.so libvulkan.so.1 vk_swiftshader_icd.json locales MEIPreload \
         PrivacySandboxAttestationsPreloaded WidevineCdm; do
    [ -e "$_out/$f" ] && cp -a "$_out/$f" "$_stage/"
done
test -x "$_stage/chrome"
"$_stage/chrome" --version || true
cd "$_base/dist"
tar -cf - "$_name" | xz -T0 -6 > "$_name.tar.xz"
rm -rf "$_name"
sha256sum "$_name.tar.xz" > "$_name.tar.xz.sha256"
ls -lh
