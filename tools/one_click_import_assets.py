#!/usr/bin/env python3
"""One-click importer for Minecraft 1.12.2 + MCP/OptiFine assets.

The importer performs a transactional import:
1. Locate a legal local Minecraft asset index and hashed objects.
2. Materialize the index into logical resource paths in a staging directory.
3. Locate an MCP 1.12.2 directory or ZIP and overlay src/assets.
4. Validate required visual, sound and OptiFine coverage.
5. Replace runtime/assets only after the staged import is valid.

No Mojang assets are bundled by this project.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile
from dataclasses import asdict, dataclass
from pathlib import Path, PurePosixPath
from typing import Iterable, Optional

REQUIRED_VISUAL = (
    Path("minecraft/lang/en_us.lang"),
    Path("minecraft/textures/gui/title/minecraft.png"),
    Path("minecraft/textures/gui/widgets.png"),
    Path("minecraft/textures/font/ascii.png"),
)

MCP_GLOBS = (
    "MCP-1.12.2*.zip",
    "MCP_1.12.2*.zip",
    "*MCP*1.12.2*.zip",
    "MCP-1.12.2*",
    "MCP_1.12.2*",
    "*MCP*1.12.2*",
)


class ImportFailure(RuntimeError):
    pass


@dataclass(frozen=True)
class ImportReport:
    minecraft_dir: str
    asset_index: str
    indexed_assets: int
    verified_hashes: int
    mcp_source: str
    mcp_assets: int
    destination: str
    visual_assets: bool
    sound_registry: bool
    sound_objects: bool
    optifine_assets: bool
    elapsed_seconds: float


def info(message: str) -> None:
    print(f"[INFO] {message}", flush=True)


def warning(message: str) -> None:
    print(f"[WARN] {message}", flush=True)


def fail(message: str) -> "NoReturn":
    raise ImportFailure(message)


def normalized(path: Path) -> Path:
    return path.expanduser().resolve()


def contains_minecraft_assets(path: Path) -> bool:
    return (path / "assets" / "indexes").is_dir() and (path / "assets" / "objects").is_dir()


def minecraft_candidates(explicit: Optional[Path]) -> list[Path]:
    candidates: list[Path] = []
    if explicit is not None:
        candidates.append(explicit)
    env_path = os.environ.get("MINECRAFT_DIR")
    if env_path:
        candidates.append(Path(env_path))
    appdata = os.environ.get("APPDATA")
    if appdata:
        candidates.append(Path(appdata) / ".minecraft")
    userprofile = os.environ.get("USERPROFILE")
    if userprofile:
        candidates.append(Path(userprofile) / "AppData" / "Roaming" / ".minecraft")
    candidates.extend(
        [
            Path.home() / ".minecraft",
            Path.home() / "Library" / "Application Support" / "minecraft",
        ]
    )
    result: list[Path] = []
    seen: set[str] = set()
    for path in candidates:
        try:
            resolved = normalized(path)
        except OSError:
            continue
        key = os.path.normcase(str(resolved))
        if key not in seen:
            seen.add(key)
            result.append(resolved)
    return result


def choose_minecraft_dir(explicit: Optional[Path], interactive: bool) -> Path:
    for candidate in minecraft_candidates(explicit):
        if contains_minecraft_assets(candidate):
            info(f"已找到 Minecraft 目录：{candidate}")
            return candidate
    if interactive:
        print("\n未自动找到有效的 .minecraft 目录。")
        entered = input("请输入 .minecraft 完整路径：").strip().strip('"')
        if entered:
            candidate = normalized(Path(entered))
            if contains_minecraft_assets(candidate):
                return candidate
    searched = "\n  ".join(str(item) for item in minecraft_candidates(explicit))
    fail(f"找不到包含 assets/indexes 与 assets/objects 的 .minecraft 目录。已检查：\n  {searched}")


def choose_asset_index(minecraft_dir: Path, requested: str) -> tuple[str, Path]:
    indexes = minecraft_dir / "assets" / "indexes"
    exact = indexes / f"{requested}.json"
    if exact.is_file():
        return requested, exact

    preferred_names = ["1.12", "1.12.2", "1.12.1"]
    for name in preferred_names:
        candidate = indexes / f"{name}.json"
        if candidate.is_file():
            warning(f"未找到 {requested}.json，自动使用 {candidate.name}")
            return name, candidate

    matching = sorted(indexes.glob("1.12*.json"), key=lambda path: path.name)
    if matching:
        candidate = matching[0]
        warning(f"自动使用资产索引 {candidate.name}")
        return candidate.stem, candidate

    available = ", ".join(path.name for path in sorted(indexes.glob("*.json"))[:20])
    fail(f"找不到 Minecraft 1.12 系列资产索引。索引目录：{indexes}\n可见索引：{available or '无'}")


def safe_zip_member(name: str) -> bool:
    pure = PurePosixPath(name)
    return not pure.is_absolute() and ".." not in pure.parts


def locate_assets_in_directory(root: Path) -> Optional[Path]:
    root = normalized(root)
    direct = [root, root / "assets", root / "src" / "assets"]
    for candidate in direct:
        if all((candidate / required).is_file() for required in REQUIRED_VISUAL):
            return candidate
    try:
        matches = list(root.glob("*/src/assets")) + list(root.glob("*/*/src/assets"))
    except OSError:
        matches = []
    for candidate in matches:
        if all((candidate / required).is_file() for required in REQUIRED_VISUAL):
            return normalized(candidate)
    return None


def zip_has_mcp_assets(path: Path) -> bool:
    try:
        with zipfile.ZipFile(path) as archive:
            names = {PurePosixPath(name) for name in archive.namelist() if safe_zip_member(name)}
            for suffix in REQUIRED_VISUAL:
                expected_tail = PurePosixPath("src/assets") / PurePosixPath(suffix.as_posix())
                if not any(tuple(name.parts[-len(expected_tail.parts):]) == expected_tail.parts for name in names):
                    return False
            return True
    except (OSError, zipfile.BadZipFile):
        return False


def mcp_candidates(project_root: Path, explicit: Optional[Path]) -> list[Path]:
    candidates: list[Path] = []
    if explicit is not None:
        candidates.append(explicit)
    search_roots = [project_root, project_root.parent, Path.cwd()]
    for search_root in search_roots:
        if not search_root.is_dir():
            continue
        for pattern in MCP_GLOBS:
            try:
                candidates.extend(search_root.glob(pattern))
            except OSError:
                pass
    result: list[Path] = []
    seen: set[str] = set()
    for path in candidates:
        try:
            resolved = normalized(path)
        except OSError:
            continue
        if resolved == project_root:
            continue
        key = os.path.normcase(str(resolved))
        if key not in seen:
            seen.add(key)
            result.append(resolved)
    return result


def choose_mcp_source(project_root: Path, explicit: Optional[Path], interactive: bool) -> Path:
    for candidate in mcp_candidates(project_root, explicit):
        if candidate.is_dir() and locate_assets_in_directory(candidate) is not None:
            info(f"已找到 MCP 目录：{candidate}")
            return candidate
        if candidate.is_file() and candidate.suffix.lower() == ".zip" and zip_has_mcp_assets(candidate):
            info(f"已找到 MCP 压缩包：{candidate}")
            return candidate
    if interactive:
        print("\n未自动找到 MCP 1.12.2 目录或压缩包。")
        entered = input("请输入 MCP-1.12.2-main 文件夹或 ZIP 完整路径：").strip().strip('"')
        if entered:
            candidate = normalized(Path(entered))
            if candidate.is_dir() and locate_assets_in_directory(candidate) is not None:
                return candidate
            if candidate.is_file() and candidate.suffix.lower() == ".zip" and zip_has_mcp_assets(candidate):
                return candidate
    fail(
        "找不到有效 MCP 1.12.2 资源。请把 MCP-1.12.2*.zip 放在项目根目录或项目文件夹旁边，"
        "也可以使用 --mcp 指定路径。"
    )


def extract_mcp_assets(zip_path: Path, target_root: Path) -> Path:
    assets_root = target_root / "mcp-assets"
    assets_root.mkdir(parents=True, exist_ok=True)
    copied = 0
    with zipfile.ZipFile(zip_path) as archive:
        for member in archive.infolist():
            name = member.filename
            if member.is_dir() or not safe_zip_member(name):
                continue
            parts = PurePosixPath(name).parts
            try:
                src_index = next(i for i in range(len(parts) - 1) if parts[i] == "src" and parts[i + 1] == "assets")
            except StopIteration:
                continue
            relative_parts = parts[src_index + 2 :]
            if not relative_parts:
                continue
            target = assets_root.joinpath(*relative_parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(member) as source, target.open("wb") as output:
                shutil.copyfileobj(source, output)
            copied += 1
    if copied == 0:
        fail(f"MCP 压缩包中没有提取到 src/assets：{zip_path}")
    assets = locate_assets_in_directory(assets_root)
    if assets is None:
        fail(f"MCP 压缩包的 src/assets 不完整：{zip_path}")
    return assets


def sha1_file(path: Path) -> str:
    digest = hashlib.sha1()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def materialize_index(index_path: Path, objects_root: Path, destination: Path, verify_hashes: bool) -> tuple[int, int]:
    try:
        data = json.loads(index_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"资产索引无法读取：{index_path}\n{exc}")
    objects = data.get("objects")
    if not isinstance(objects, dict) or not objects:
        fail(f"资产索引没有有效 objects：{index_path}")

    copied = 0
    verified = 0
    missing: list[str] = []
    mismatched: list[str] = []
    for logical, metadata in objects.items():
        if not isinstance(logical, str) or not isinstance(metadata, dict):
            continue
        digest = metadata.get("hash")
        if not isinstance(digest, str) or len(digest) != 40:
            fail(f"资产索引含无效 SHA-1：{logical}")
        source = objects_root / digest[:2] / digest
        if not source.is_file():
            missing.append(str(source))
            continue
        if verify_hashes:
            actual = sha1_file(source)
            if actual.lower() != digest.lower():
                mismatched.append(f"{logical}: expected {digest}, got {actual}")
                continue
            verified += 1
        target = destination / Path(*PurePosixPath(logical).parts)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        copied += 1

    if missing:
        preview = "\n  ".join(missing[:12])
        fail(f"资产索引引用了 {len(missing)} 个缺失对象。前若干项：\n  {preview}")
    if mismatched:
        preview = "\n  ".join(mismatched[:12])
        fail(f"发现 {len(mismatched)} 个 SHA-1 不匹配对象。前若干项：\n  {preview}")
    return copied, verified


def copy_merge(source: Path, destination: Path) -> int:
    copied = 0
    for path in source.rglob("*"):
        relative = path.relative_to(source)
        target = destination / relative
        if path.is_dir():
            target.mkdir(parents=True, exist_ok=True)
        elif path.is_file():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)
            copied += 1
    return copied


def validate_assets(root: Path) -> dict[str, bool]:
    missing = [root / item for item in REQUIRED_VISUAL if not (root / item).is_file()]
    if missing:
        fail("导入结果缺少必要视觉资源：\n  " + "\n  ".join(str(path) for path in missing))
    namespace = root / "minecraft"
    coverage = {
        "visual_assets": True,
        "sound_registry": (namespace / "sounds.json").is_file(),
        "sound_objects": (namespace / "sounds").is_dir() and any((namespace / "sounds").rglob("*.ogg")),
        "optifine_assets": (namespace / "optifine").is_dir() or (namespace / "mcpatcher").is_dir(),
    }
    incomplete = [key for key, value in coverage.items() if not value]
    if incomplete:
        fail("导入结果覆盖不完整：" + ", ".join(incomplete))
    return coverage


def atomic_replace(staging: Path, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    backup = destination.with_name(destination.name + ".previous")
    if backup.exists():
        shutil.rmtree(backup)
    if destination.exists():
        destination.replace(backup)
    try:
        staging.replace(destination)
    except Exception:
        if destination.exists():
            shutil.rmtree(destination)
        if backup.exists():
            backup.replace(destination)
        raise
    else:
        if backup.exists():
            shutil.rmtree(backup)


def run_cargo_validation(project_root: Path, destination: Path) -> None:
    cargo = shutil.which("cargo")
    if cargo is None:
        warning("未找到 cargo，已跳过 Rust 侧 validate-assets；Python 完整性验证已通过。")
        return
    validator_manifest = project_root / "tools" / "asset-validator" / "Cargo.toml"
    if not validator_manifest.is_file():
        fail(f"缺少独立资源验证器：{validator_manifest}")
    command = [
        cargo,
        "run",
        "-q",
        "--manifest-path",
        str(validator_manifest),
        "--",
        "--path",
        str(destination),
    ]
    info("正在执行隔离的 Rust 资源验证器……")
    result = subprocess.run(command, cwd=project_root, check=False)
    if result.returncode != 0:
        fail(f"Rust 侧 validate-assets 失败，退出码 {result.returncode}")


def write_report(path: Path, report: ImportReport) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(asdict(report), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def parse_args() -> argparse.Namespace:
    default_root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description="一键导入 Minecraft 1.12.2 与 MCP/OptiFine 资源")
    parser.add_argument("--project-root", type=Path, default=default_root)
    parser.add_argument("--minecraft-dir", type=Path)
    parser.add_argument("--mcp", type=Path, help="MCP 文件夹、src/assets 文件夹或 ZIP")
    parser.add_argument("--index", default="1.12")
    parser.add_argument("--destination", type=Path)
    parser.add_argument("--no-hash-check", action="store_true", help="跳过官方对象 SHA-1 校验")
    parser.add_argument("--non-interactive", action="store_true")
    parser.add_argument("--skip-cargo-validation", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    started = time.monotonic()
    project_root = normalized(args.project_root)
    if not (project_root / "Cargo.toml").is_file() or not (project_root / "tools").is_dir():
        fail(f"不是有效项目根目录：{project_root}")
    destination = normalized(args.destination) if args.destination else project_root / "runtime" / "assets"
    interactive = not args.non_interactive and sys.stdin.isatty()

    minecraft_dir = choose_minecraft_dir(args.minecraft_dir, interactive)
    index_name, index_path = choose_asset_index(minecraft_dir, args.index)
    mcp_source = choose_mcp_source(project_root, args.mcp, interactive)

    runtime_root = destination.parent
    runtime_root.mkdir(parents=True, exist_ok=True)
    temp_parent = runtime_root / ".asset-import-work"
    if temp_parent.exists():
        shutil.rmtree(temp_parent)
    temp_parent.mkdir(parents=True, exist_ok=True)

    try:
        staging = temp_parent / "assets-staging"
        staging.mkdir(parents=True, exist_ok=True)
        info(f"正在物化官方资产索引 {index_path.name}……")
        indexed_assets, verified_hashes = materialize_index(
            index_path,
            minecraft_dir / "assets" / "objects",
            staging,
            verify_hashes=not args.no_hash_check,
        )
        info(f"已导入 {indexed_assets} 个官方索引资源。")

        if mcp_source.is_file():
            info("正在从 MCP ZIP 提取 src/assets……")
            mcp_assets_root = extract_mcp_assets(mcp_source, temp_parent / "mcp-extracted")
        else:
            mcp_assets_root = locate_assets_in_directory(mcp_source)
            if mcp_assets_root is None:
                fail(f"MCP 目录没有完整 src/assets：{mcp_source}")

        info(f"正在覆盖合并 MCP/OptiFine 资源：{mcp_assets_root}")
        mcp_assets = copy_merge(mcp_assets_root, staging)
        coverage = validate_assets(staging)
        info("暂存资源完整性验证通过。")

        atomic_replace(staging, destination)
        info(f"资源已安装到：{destination}")

        if not args.skip_cargo_validation:
            run_cargo_validation(project_root, destination)

        elapsed = round(time.monotonic() - started, 3)
        report = ImportReport(
            minecraft_dir=str(minecraft_dir),
            asset_index=index_name,
            indexed_assets=indexed_assets,
            verified_hashes=verified_hashes,
            mcp_source=str(mcp_source),
            mcp_assets=mcp_assets,
            destination=str(destination),
            elapsed_seconds=elapsed,
            **coverage,
        )
        report_path = destination.parent / "asset-import-report.json"
        write_report(report_path, report)
        print("\n========== 导入成功 ==========")
        print(f"官方索引资源：{indexed_assets}")
        print(f"SHA-1 已验证：{verified_hashes}")
        print(f"MCP/OptiFine 资源：{mcp_assets}")
        print(f"目标目录：{destination}")
        print(f"报告文件：{report_path}")
        print(f"耗时：{elapsed:.3f} 秒")
        return 0
    finally:
        if temp_parent.exists():
            shutil.rmtree(temp_parent, ignore_errors=True)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ImportFailure as exc:
        print(f"\n[ERROR] {exc}", file=sys.stderr)
        raise SystemExit(2)
    except KeyboardInterrupt:
        print("\n[ERROR] 用户取消。", file=sys.stderr)
        raise SystemExit(130)
