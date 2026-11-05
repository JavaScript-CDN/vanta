# Vanta

Local, single-user multi-profile browser. This repository currently holds the
**engine build pipeline**: a patched Chromium compiled on GitHub's free runners.

## Engine

| | |
|---|---|
| Chromium | `engine/chromium_version.txt` (157.0.8103.38, stable) |
| Source check | SHA-256 pinned in `engine/chromium_sha256.txt` |
| Build flags | `engine/args.gn` (official/optimized build, Chrome codecs, no Google API keys, Safe Browsing on) |
| Patches | `patches/series` + `patches/*.patch`, applied with `patch -p1 --fuzz=0` |
| Output | artifact `vanta-engine-linux-x64` (`.tar.xz` + `.sha256`) |

### How the build runs
`.github/workflows/engine.yml` (manual run or tag `engine-v*`):
1. **prep** – builds the pinned Debian builder image, downloads and verifies the
   Chromium tarball, applies patches, fetches Chromium's own clang/rust/sysroot,
   runs `gn gen`.
2. **Compile stage 1…12** – each job restores the build tree, runs `ninja` for at
   most 4h50m, then saves the tree for the next job. The chain stops as soon as
   `chrome` links; free runners have a 6-hour job limit, so a full build takes
   several stages (~15–20 h on 4 cores).
3. The final stage packages the runtime files and uploads them.

### Adding a patch
Put `NNNN-name.patch` in `patches/`, add its name to `patches/series`, push, and
run the workflow. Any patch that does not apply cleanly stops the build in `prep`.

### Security
- Every third-party Action is pinned to a commit SHA; workflow token is read-only.
- Builder base image pinned by digest.
- No telemetry endpoints or API keys are compiled in.
