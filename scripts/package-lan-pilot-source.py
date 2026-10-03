#!/usr/bin/env python3
"""Package the reviewed LAN pilot source without ignored local credentials."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
from zipfile import ZIP_DEFLATED, ZipFile


ROOT = Path(__file__).resolve().parents[1]
VERSION = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))["version"]
EXTRA = {
    ".gitignore",
    "docs/piloto-mac-windows.md",
    "scripts/build-lan-pilot-windows.ps1",
    "scripts/package-lan-pilot-source.py",
    "src-tauri/tauri.pilot.conf.json",
    "src-tauri/tauri.pilot.auto.conf.json",
    "src-tauri/macos/Info.pilot.plist",
    "src-tauri/windows/pilot-installer-hooks.nsh",
    "src/components/board/ReceptionLinkCard.tsx",
}
tracked = {
    name.decode("utf-8")
    for name in subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).split(b"\0")
    if name
}
output = ROOT / "pilot-artifacts" / f"nightdesk-lan-pilot-source-{VERSION}.zip"
output.parent.mkdir(exist_ok=True)
prefix = f"nightdesk-lan-pilot-{VERSION}/"

with ZipFile(output, "w", compression=ZIP_DEFLATED, compresslevel=6) as archive:
    for name in sorted(tracked | EXTRA):
        path = ROOT / name
        if not path.is_file() or path.is_symlink():
            raise RuntimeError(f"Missing or unsafe source path: {name}")
        if "embedded_device.local" in name or name.startswith("pilot-artifacts/"):
            raise RuntimeError(f"Unsafe source path: {name}")
        archive.write(path, prefix + name)

with output.open("rb") as source:
    digest = hashlib.sha256()
    for chunk in iter(lambda: source.read(1024 * 1024), b""):
        digest.update(chunk)
sha256 = digest.hexdigest()
print(f"{output}\nSHA-256: {sha256}")
