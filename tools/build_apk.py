#!/usr/bin/env python3
"""Build and sign the Minecraft 1.12.2 Rust Android APK.

Stages runtime/assets (minus sounds) -> writes mcassets.list -> aapt2 link ->
zipalign -> apksigner (via java -jar). Uses the Android SDK installed on this
machine. Run from the repository root: python tools/build_apk.py
"""
import os
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DIST = ROOT / "dist"
STAGE = DIST / "stage"
ASSETS_STAGE = STAGE / "mcassets"
LIST_FILE = STAGE / "mcassets.list"

# Android builds have no audio backend; ship the visual assets only.
EXCLUDE_DIRS = {"minecraft/sounds"}
EXCLUDE_FILES = {"minecraft/sounds.json"}


def find_sdk() -> pathlib.Path:
    env = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    local = pathlib.Path(os.environ.get("LOCALAPPDATA", "")) / "Android" / "Sdk"
    for candidate in (pathlib.Path(env) if env else None, local):
        if candidate and (candidate / "build-tools").is_dir():
            return candidate
    raise SystemExit("Android SDK not found (set ANDROID_HOME)")


def stage_assets(src: pathlib.Path) -> None:
    shutil.rmtree(ASSETS_STAGE, ignore_errors=True)
    ASSETS_STAGE.mkdir(parents=True)
    entries = []
    for path in sorted(src.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(src).as_posix()
        if rel in EXCLUDE_FILES or any(rel.startswith(d + "/") for d in EXCLUDE_DIRS):
            continue
        target = ASSETS_STAGE / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        entries.append(rel)
    LIST_FILE.write_text("\n".join(entries) + "\n", encoding="utf-8")
    print(f"[APK] staged {len(entries)} asset files")


def tool(sdk: pathlib.Path, name: str) -> pathlib.Path:
    build_tools = sorted((sdk / "build-tools").iterdir(), reverse=True)[0]
    return build_tools / name


def platform_jar(sdk: pathlib.Path) -> pathlib.Path:
    platforms = sorted((sdk / "platforms").iterdir(), reverse=True)
    if not platforms:
        raise SystemExit("no android platforms installed")
    return platforms[0] / "android.jar"


def run(cmd):
    print("[APK]", " ".join(str(c) for c in cmd))
    subprocess.run([str(c) for c in cmd], check=True)


def add_native_libs(apk: pathlib.Path) -> None:
    """Append the cdylib at lib/<abi>/ for NativeActivity to load."""
    import zipfile
    so_src = ROOT / "target" / "android" / "arm64-v8a" / "libminecraft_1_12_2_rust_vulkan.so"
    if not so_src.is_file():
        raise SystemExit("native lib not built; run cargo ndk ... first")
    with zipfile.ZipFile(apk, "a", zipfile.ZIP_DEFLATED) as archive:
        archive.write(so_src, "lib/arm64-v8a/libminecraft_1_12_2_rust_vulkan.so")
    print(f"[APK] added native lib ({so_src.stat().st_size // 1024} KiB)")


def debug_keystore() -> pathlib.Path:
    """Reuse the standard Android debug keystore, or generate one via keytool."""
    standard = pathlib.Path(os.environ.get("USERPROFILE", "")) / ".android" / "debug.keystore"
    if standard.is_file():
        return standard
    keystore = DIST / "debug.keystore"
    if keystore.is_file():
        return keystore
    keytool = shutil.which("keytool")
    if keytool is None:
        # Locate keytool through the active JVM's java.home.
        try:
            out = subprocess.run(["java", "-XshowSettings:properties", "-version"],
                                 capture_output=True, text=True, check=True).stderr
            for line in out.splitlines():
                line = line.strip()
                if line.startswith("java.home"):
                    home = line.split("=", 1)[1].strip()
                    candidate = pathlib.Path(home) / "bin" / "keytool.exe"
                    if candidate.is_file():
                        keytool = str(candidate)
                        break
        except (OSError, subprocess.CalledProcessError):
            pass
    if keytool is None:
        raise SystemExit("keytool not found; install a JDK or add it to PATH")
    run([keytool, "-genkeypair", "-v", "-keystore", keystore, "-alias", "androiddebugkey",
         "-keyalg", "RSA", "-keysize", "2048", "-validity", "10000",
         "-storepass", "android", "-keypass", "android",
         "-dname", "CN=Android Debug,O=Android,C=US"])
    return keystore


def main() -> None:
    assets_src = ROOT / "runtime" / "assets"
    if not (assets_src / "minecraft").is_dir():
        raise SystemExit("runtime/assets not imported yet")
    sdk = find_sdk()
    aapt2 = tool(sdk, "aapt2.exe")
    zipalign = tool(sdk, "zipalign.exe")
    # apksigner ships as a .bat wrapper on Windows, which subprocess cannot
    # execute directly; invoke its jar through java instead.
    apksigner_jar = tool(sdk, "lib") / "apksigner.jar"
    jar = platform_jar(sdk)

    stage_assets(assets_src)
    DIST.mkdir(exist_ok=True)

    unaligned = DIST / "base.unaligned.apk"
    run([aapt2, "link", "-o", unaligned, "-I", jar, "--manifest", ROOT / "AndroidManifest.xml",
         "--min-sdk-version", "31", "--target-sdk-version", "34",
         "--version-code", "1", "--version-name", "0.1", "-A", STAGE])

    add_native_libs(unaligned)

    aligned = DIST / "base.aligned.apk"
    run([zipalign, "-f", "4", unaligned, aligned])

    keystore = debug_keystore()

    final = DIST / "Minecraft112Rust.apk"
    run(["java", "-jar", apksigner_jar, "sign", "--ks", keystore, "--ks-pass", "pass:android",
         "--out", final, aligned])
    print(f"[APK] done: {final} ({final.stat().st_size // (1024 * 1024)} MiB)")


if __name__ == "__main__":
    main()
