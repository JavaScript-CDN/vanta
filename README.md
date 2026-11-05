# Vanta

Local, single-user multi-profile browser for **Windows**. This repository currently holds
the **engine build pipeline**: Chromium compiled for Windows x64 on GitHub's free runners.

## Engine

| | |
|---|---|
| Chromium | `engine/chromium_version.txt` (157.0.8103.38, current stable) |
| Source | Google's official git (`chromium.googlesource.com`), exact release tag, deps via `depot_tools` pinned by Chromium's own DEPS |
| Toolchain | Google's clang/Rust/Node fetched by Chromium's hooks + Visual Studio 2026 + Windows SDK 10.0.28000 (signature-checked) |
| Build flags | `engine/args.win.gn`: official build (ThinLTO + PGO, like Chrome), Chrome codecs, real version string, no Google API keys, Safe Browsing default |
| Patches | `patches/series` + `patches/*.patch`, applied with `patch -p1 --fuzz=0` (any mismatch stops the build) |
| Output | artifact `vanta-engine-win-x64` (`.zip` + `.sha256`) |

### How the build runs
`.github/workflows/engine-windows.yml` (manual run or tag `engine-v*`), runner `windows-2025-vs2026`:
1. **prep** – clone the release tag, `gclient sync` (Windows deps + hooks), apply patches,
   `gn gen --fail-on-unused-args`, save the tree.
2. **Compile stage 1…12** – each job restores the tree, runs `ninja -j3` for at most 4 h, saves
   the tree. Free runners stop jobs at 6 h, so a full Windows build takes ~8 stages (~35–40 h).
3. The stage that links `chrome.exe` packages the runtime files and uploads them.

### Security
- Every third-party Action is pinned to a full commit SHA; the workflow token is read-only and
  not persisted in the checkout.
- Chromium comes only from Google's own servers at an exact tag; the Windows SDK installer's
  Authenticode signature must be Microsoft's.
- No telemetry API keys are compiled in.
