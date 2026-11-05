#!/usr/bin/env python3
"""Vanta engine: build patched Chromium for Windows x64 on GitHub-hosted runners.

Subcommands (all run on a Windows runner):
  prepare  - fetch Chromium + all Windows deps with depot_tools (Google's documented
             path), run hooks (Google clang, Rust, Node, PGO profiles), apply Vanta
             patches, write args.gn, run gn gen.
  stage    - run ninja for at most --timeout seconds. Exit 0 = done, 124 = time box hit.
  package  - collect the runtime files into dist/vanta-engine-<ver>-win-x64.zip + .sha256

The staged layout (time-boxed ninja, tree carried to the next job as an artifact) follows
ungoogled-chromium-windows (BSD-3-Clause); see THIRD_PARTY_NOTICES.md.
"""
import argparse
import ctypes
import hashlib
import os
import re
import shutil
import stat
import subprocess
import sys
import time
import zipfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
WORK = Path(os.environ.get("VANTA_WORK", r"C:\v"))   # short path: avoids MAX_PATH issues
SRC = WORK / "src"
OUT = SRC / "out" / "Default"
DEPOT = WORK / "depot_tools"
VERSION = (REPO / "engine" / "chromium_version.txt").read_text().strip()
TIMEOUT_EXIT = 124


def log(msg):
    print(f"[vanta] {msg}", flush=True)


def build_env():
    env = os.environ.copy()
    env["PATH"] = str(DEPOT) + os.pathsep + env["PATH"]
    env["DEPOT_TOOLS_UPDATE"] = "0"              # stay on the commit Chromium pins
    env["DEPOT_TOOLS_WIN_TOOLCHAIN"] = "0"       # use the locally installed Visual Studio
    env["DEPOT_TOOLS_METRICS"] = "0"
    env["DEPOT_TOOLS_REPORT_BUILD"] = "0"
    env["GCLIENT_SUPPRESS_GIT_VERSION_WARNING"] = "1"
    env["PYTHONUNBUFFERED"] = "1"
    for var in ("TMP", "TEMP"):
        Path(env[var]).mkdir(parents=True, exist_ok=True)
    return env


def run(cmd, cwd=None, env=None):
    log("$ " + " ".join(map(str, cmd)))
    subprocess.run([str(c) for c in cmd], cwd=cwd, env=env or build_env(), check=True)


def depot_tools_commit():
    deps = (SRC / "DEPS").read_text(encoding="utf-8")
    m = re.search(r"depot_tools\.git'\s*\+\s*'@'\s*\+\s*'([0-9a-f]{40})'", deps)
    if not m:
        sys.exit("could not find the depot_tools commit pinned in DEPS")
    return m.group(1)


def _rm_readonly(func, path, _exc):
    os.chmod(path, stat.S_IWRITE)
    func(path)


def prepare(_args):
    WORK.mkdir(parents=True, exist_ok=True)
    env = build_env()

    # 1. Chromium src at the exact release tag (shallow).
    if not (SRC / "DEPS").exists():
        run(["git", "clone", "--depth=1", "--branch", VERSION, "-c", "advice.detachedHead=false",
             "-c", "core.longpaths=true", "https://chromium.googlesource.com/chromium/src.git",
             SRC], env=env)
    head = subprocess.run(["git", "-C", str(SRC), "describe", "--tags", "--exact-match"],
                          capture_output=True, text=True, env=env).stdout.strip()
    if head != VERSION:
        sys.exit(f"src is at '{head}', expected tag {VERSION}")
    log(f"Chromium src at tag {head}")

    # 2. depot_tools at the commit Chromium itself pins in DEPS.
    if not (DEPOT / "gclient.py").exists():
        commit = depot_tools_commit()
        DEPOT.mkdir(exist_ok=True)
        run(["git", "init", "-q"], cwd=DEPOT, env=env)
        run(["git", "remote", "add", "origin",
             "https://chromium.googlesource.com/chromium/tools/depot_tools.git"], cwd=DEPOT, env=env)
        run(["git", "fetch", "--depth=1", "origin", commit], cwd=DEPOT, env=env)
        run(["git", "checkout", "-q", "FETCH_HEAD"], cwd=DEPOT, env=env)
    # Bootstrap depot_tools' own CIPD client, python3 and git without moving off the pin.
    run(["cmd", "/c", "call cipd_bin_setup.bat && call bootstrap\\win_tools.bat"], cwd=DEPOT, env=env)
    run([DEPOT / "gclient.bat", "--version"], cwd=WORK, env=env)

    # 3. All Windows deps + hooks (Google clang, Rust, Node, gn, ninja, PGO profiles, LASTCHANGE).
    (WORK / ".gclient").write_text(
        'solutions = [{\n'
        '  "name": "src",\n'
        '  "url": "https://chromium.googlesource.com/chromium/src.git",\n'
        '  "managed": False,\n'
        '  "custom_deps": {},\n'
        '  "custom_vars": {"checkout_configuration": "small", "checkout_pgo_profiles": True},\n'
        '}]\n'
        'target_os = ["win"]\n'
        'target_os_only = True\n', encoding="utf-8")
    run([DEPOT / "gclient.bat", "sync", "--no-history", "--shallow", "-D", "--force",
         "--nohooks", "--jobs", "8"], cwd=WORK, env=env)
    # Some hooks (e.g. testing/generate_location_tags.py -> dirmd.exe) call tools from the
    # in-tree copy src/third_party/depot_tools, whose CIPD binaries are not bootstrapped by
    # sync. Bootstrap them (same DEPS-pinned commit, no self-update), then run every hook.
    run(["cmd", "/c", "call cipd_bin_setup.bat"], cwd=SRC / "third_party" / "depot_tools", env=env)
    dirmd = SRC / "third_party" / "depot_tools" / ".cipd_bin" / "dirmd.exe"
    if not dirmd.exists():
        sys.exit(f"{dirmd} missing after CIPD bootstrap")
    run([DEPOT / "gclient.bat", "runhooks", "--jobs", "8"], cwd=WORK, env=env)

    # 4. Drop git metadata; the build itself does not need it (release tarballs have none).
    for git_dir in sorted(SRC.rglob(".git"), key=lambda p: len(p.parts), reverse=True):
        if git_dir.is_dir():
            shutil.rmtree(git_dir, onerror=_rm_readonly)
        else:
            git_dir.unlink()

    apply_patches()
    write_args()
    run([DEPOT / "gn.bat", "gen", "out\\Default", "--fail-on-unused-args"], cwd=SRC, env=env)
    run([DEPOT / "gn.bat", "args", "out\\Default", "--list", "--short", "--overrides-only"],
        cwd=SRC, env=env)


def apply_patches():
    patch_exe = Path(r"C:\Program Files\Git\usr\bin\patch.exe")
    for line in (REPO / "patches" / "series").read_text(encoding="utf-8").splitlines():
        name = line.split("#", 1)[0].strip()
        if not name:
            continue
        log(f"applying patches/{name}")
        with open(REPO / "patches" / name, "rb") as fh:
            subprocess.run([str(patch_exe), "-p1", "--forward", "--fuzz=0", "--binary",
                            "--no-backup-if-mismatch", "-d", str(SRC)], stdin=fh, check=True)


def write_args():
    OUT.mkdir(parents=True, exist_ok=True)
    args = (REPO / "engine" / "args.win.gn").read_text(encoding="utf-8")
    (OUT / "args.gn").write_text(args, encoding="utf-8")
    log("args.gn:\n" + args)


def stage(args):
    env = build_env()
    ninja = SRC / "third_party" / "ninja" / "ninja.exe"
    if not ninja.exists():
        sys.exit(f"missing {ninja}")
    cmd = [str(ninja), "-j", str(args.jobs), "-C", "out\\Default", "chrome", "chromedriver"]
    log("$ " + " ".join(cmd) + f"   (time box {args.timeout}s)")
    proc = subprocess.Popen(cmd, cwd=SRC, env=env,
                            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP)
    try:
        rc = proc.wait(timeout=args.timeout)
    except subprocess.TimeoutExpired:
        log("time box reached, stopping ninja cleanly")
        for _ in range(3):
            ctypes.windll.kernel32.GenerateConsoleCtrlEvent(1, proc.pid)  # CTRL_BREAK
            time.sleep(2)
        try:
            proc.wait(timeout=180)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
        sys.exit(TIMEOUT_EXIT)
    if rc != 0:
        sys.exit(rc)
    if not (OUT / "chrome.exe").exists() or not (OUT / "chrome.dll").exists():
        sys.exit("ninja finished but chrome.exe / chrome.dll is missing")
    log("build complete")


RUNTIME = ["chrome.exe", "chrome.dll", "chrome_elf.dll", "chrome_proxy.exe", "chrome_pwa_launcher.exe",
           "chrome_wer.dll", "chromedriver.exe", "elevation_service.exe", "notification_helper.exe",
           "chrome_100_percent.pak", "chrome_200_percent.pak", "resources.pak", "icudtl.dat",
           "v8_context_snapshot.bin", "snapshot_blob.bin", "libEGL.dll", "libGLESv2.dll",
           "vk_swiftshader.dll", "vk_swiftshader_icd.json", "vulkan-1.dll", "d3dcompiler_47.dll",
           "dxcompiler.dll", "dxil.dll", "chrome.VisualElementsManifest.xml", "locales",
           "MEIPreload", "PrivacySandboxAttestationsPreloaded"]


def package(_args):
    dist = REPO / "dist"
    dist.mkdir(exist_ok=True)
    name = f"vanta-engine-{VERSION}-win-x64"
    zpath = dist / f"{name}.zip"
    for required in ("chrome.exe", "chrome.dll", "chrome_elf.dll", "resources.pak", "icudtl.dat"):
        if not (OUT / required).exists():
            sys.exit(f"missing required runtime file {required}")
    with zipfile.ZipFile(zpath, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as z:
        for item in RUNTIME:
            p = OUT / item
            if p.is_file():
                z.write(p, f"{name}/{item}")
            elif p.is_dir():
                for f in p.rglob("*"):
                    if f.is_file():
                        z.write(f, f"{name}/{f.relative_to(OUT).as_posix()}")
    digest = hashlib.sha256(zpath.read_bytes()).hexdigest()
    (dist / f"{name}.zip.sha256").write_text(f"{digest}  {zpath.name}\n")
    log(f"packaged {zpath.name} ({zpath.stat().st_size // (1 << 20)} MiB) sha256={digest}")


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("prepare")
    s = sub.add_parser("stage")
    s.add_argument("--timeout", type=int, default=14400)
    s.add_argument("--jobs", type=int, default=3)
    sub.add_parser("package")
    a = ap.parse_args()
    {"prepare": prepare, "stage": stage, "package": package}[a.cmd](a)


if __name__ == "__main__":
    main()
