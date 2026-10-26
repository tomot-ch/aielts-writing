#!/usr/bin/env python3
"""Package the macOS app with the complete, hash-verified question collection.

Only writes a new output directory. It never installs or launches the app.
"""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import plistlib
import re
import shutil
import struct
import subprocess
import sys
import zipfile


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def run(*args):
    subprocess.run(list(map(str, args)), check=True)


def text_sha256(path):
    """Hash executable instructions; re-signing changes the signature bytes."""
    data = path.read_bytes()
    if struct.unpack_from("<I", data)[0] != 0xFEEDFACF:
        raise ValueError("Expected a thin 64-bit Mach-O executable")
    offset = 32
    for _ in range(struct.unpack_from("<I", data, 16)[0]):
        command, size = struct.unpack_from("<II", data, offset)
        if command == 0x19:  # LC_SEGMENT_64
            count = struct.unpack_from("<I", data, offset + 64)[0]
            for i in range(count):
                section = offset + 72 + i * 80
                if data[section:section + 16].rstrip(b"\0") == b"__text":
                    length = struct.unpack_from("<Q", data, section + 40)[0]
                    start = struct.unpack_from("<I", data, section + 48)[0]
                    return hashlib.sha256(data[start:start + length]).hexdigest()
        offset += size
    raise ValueError("Executable has no __text section")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assets", type=Path, required=True,
                        help="Directory containing the published ZIPs and catalog")
    parser.add_argument("--output", type=Path, required=True,
                        help="A new directory, which must not exist")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("This packager requires macOS (ditto, codesign, hdiutil).")
    assets = args.assets.resolve()
    output = args.output.resolve()
    if output.exists():
        parser.error("Output already exists; choose a new directory.")

    sums = {}
    for line in (assets / "SHA256SUMS.txt").read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        sums[name.lstrip("*")] = digest
    catalog = json.loads((assets / "question-catalog.json").read_text())
    questions = catalog["questions"]
    if len(questions) != 156 or Counter(q["examType"] for q in questions) != {
        "academic": 128, "general_training": 28
    }:
        raise ValueError("Expected the 128 Academic + 28 General Training collection")

    archive_names = {
        "academic": "ielts-writing-questions.zip",
        "general_training": "ielts-writing-general-training.zip",
    }
    app_zip = assets / "aiELTS-Writing-v2.0.0-macos-arm64.zip"
    for path in [app_zip, assets / "question-catalog.json",
                 *(assets / name for name in archive_names.values())]:
        if sha256(path) != sums.get(path.name):
            raise ValueError(f"Published input checksum mismatch: {path.name}")

    # Validate every question before writing or replacing any bundle contents.
    images = {}
    for exam, archive_name in archive_names.items():
        with zipfile.ZipFile(assets / archive_name) as archive:
            if archive.testzip() is not None:
                raise ValueError(f"Corrupt archive: {archive_name}")
            for question in (q for q in questions if q["examType"] == exam):
                entries = [name for name in archive.namelist()
                           if name.endswith("/" + question["fileName"])]
                if len(entries) != 1:
                    raise ValueError(f"Missing or ambiguous question: {question['id']}")
                content = archive.read(entries[0])
                if hashlib.sha256(content).hexdigest() != question["sha256"]:
                    raise ValueError(f"Question checksum mismatch: {question['id']}")
                if not content.startswith(b"\x89PNG\r\n\x1a\n"):
                    raise ValueError(f"Question is not PNG: {question['id']}")
                test = str(question["test"])
                if not re.fullmatch(r"[A-Za-z0-9]+", test) or question["task"] not in (1, 2):
                    raise ValueError("Invalid question task or test number")
                label = "学术类" if exam == "academic" else "培训类"
                name = f"剑雅{int(question['volume']):02d}_{label}_Test{test}_Task{question['task']}.png"
                if name in images:
                    raise ValueError(f"Duplicate bundled filename: {name}")
                images[name] = content
                question["bundledFileName"] = name
    if len(images) != 156 or len({q["sha256"] for q in questions}) != 156:
        raise ValueError("Question collection contains duplicates")

    with zipfile.ZipFile(app_zip) as archive:
        if archive.testzip() is not None:
            raise ValueError("Corrupt application ZIP")
    output.mkdir(parents=True)
    stage = output / "staging"
    stage.mkdir()
    run("ditto", "-x", "-k", app_zip, stage)
    apps = list(stage.glob("*.app"))
    if len(apps) != 1:
        raise ValueError("Expected exactly one application")
    app = apps[0]
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    if info["CFBundleIdentifier"] != "com.local.ieltswriting" or info["CFBundleShortVersionString"] != "2.0.0":
        raise ValueError("Unexpected application identity or version")
    run("codesign", "--verify", "--deep", "--strict", app)
    original_binary_hash = sha256(app / "Contents/MacOS/ielts-writing")
    instructions_hash = text_sha256(app / "Contents/MacOS/ielts-writing")

    resources = app / "Contents/Resources/resources/questions"
    shutil.rmtree(resources)
    resources.mkdir()
    for name, content in sorted(images.items()):
        (resources / name).write_bytes(content)
    (resources / "question-catalog.json").write_text(
        json.dumps(catalog, ensure_ascii=False, indent=2) + "\n")
    (resources / "SHA256SUMS.txt").write_text("".join(
        f"{hashlib.sha256(content).hexdigest()}  {name}\n"
        for name, content in sorted(images.items())))
    (resources / "README.md").write_text(
        "# 爱雅思写作 · 完整题库\n\n"
        "剑雅 4–19 学术类 128 题、培训类 28 题，共 156 题。\n"
        "首次设置数据目录后由应用自动导入，无需解压或手工选择图片。\n"
        "相同图片按内容去重；保留用户自行导入的题目与回收站删除记录。\n"
        "来源与校验见 question-catalog.json。题目版权归原权利人所有。\n")
    run("codesign", "--force", "--sign", "-", "--identifier",
        info["CFBundleIdentifier"], "--timestamp=none", app)
    run("codesign", "--verify", "--deep", "--strict", app)
    if text_sha256(app / "Contents/MacOS/ielts-writing") != instructions_hash:
        raise ValueError("Executable instructions unexpectedly changed")

    stem = "aiELTS-Writing-v2.0.0-macos-arm64-full"
    run("ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", app, output / f"{stem}.zip")
    (stage / "Applications").symlink_to("/Applications")
    run("hdiutil", "create", "-volname", "爱雅思写作 完整题库版",
        "-srcfolder", stage, "-format", "UDZO", output / f"{stem}.dmg")
    run("hdiutil", "verify", output / f"{stem}.dmg")
    packages = [output / f"{stem}.dmg", output / f"{stem}.zip"]
    (output / "SHA256SUMS.txt").write_text("".join(
        f"{sha256(path)}  {path.name}\n" for path in packages))
    (output / "使用说明.md").write_text(
        "# 爱雅思写作 · 完整题库版\n\n"
        "应用 v2.0.0，已包含学术类 128 题与培训类 28 题。\n\n"
        "1. 打开 DMG，把应用拖入“应用程序”（也可解压 ZIP 安装，二选一）。\n"
        "2. 打开应用，确认长期保存作文的数据目录。\n"
        "3. 题目自动入库；从题库选择题目即可开始写作。\n\n"
        "无需另下题包或手动导图。已有作文与自导入题目保留，相同图片不重复导入。\n"
        "提供内置 Prompt；AI 功能仍需自行配置接口、API Key 和模型。\n"
        "适用于 M 系列 Mac，采用 ad hoc 临时签名、未经 Apple 公证。\n"
        "首次打开说明：https://github.com/tomot-ch/aielts-writing/blob/main/docs/INSTALL.md\n\n"
        "公开下载与版本说明以 GitHub Release 为准。\n")
    report = {
        "version": "2.0.0", "questionCount": 156, "academicCount": 128,
        "generalTrainingCount": 28,
        "originalExecutableSha256": original_binary_hash,
        "executableSha256": sha256(app / "Contents/MacOS/ielts-writing"),
        "executableInstructionsSha256": instructions_hash,
        "app": str(app), "questionResources": str(resources),
        "packages": {p.name: {"size": p.stat().st_size, "sha256": sha256(p)} for p in packages},
    }
    (output / "build-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False, indent=2), flush=True)


if __name__ == "__main__":
    main()
