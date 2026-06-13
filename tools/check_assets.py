#!/usr/bin/env python3
from pathlib import Path
import subprocess
import sys
from typing import Optional

ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN_SUFFIXES = {
    ".7z",
    ".aif",
    ".aiff",
    ".avi",
    ".bin",
    ".bmp",
    ".bz2",
    ".dmg",
    ".flac",
    ".gif",
    ".gz",
    ".heic",
    ".icns",
    ".ico",
    ".img",
    ".iso",
    ".jpeg",
    ".jpg",
    ".m4a",
    ".mid",
    ".midi",
    ".mkv",
    ".mov",
    ".mp3",
    ".mp4",
    ".ogg",
    ".pdf",
    ".png",
    ".rar",
    ".raw",
    ".rom",
    ".sit",
    ".snd",
    ".svg",
    ".syx",
    ".tar",
    ".tgz",
    ".tif",
    ".tiff",
    ".wav",
    ".webp",
    ".xz",
    ".zip",
}
ALLOWLIST = {
    # "path/to/generated-fixture.wav": "Synthetic test fixture generated from repo-owned code.",
}
BLOCKED_TRACKED_PREFIXES = (
    "captures/",
    "firmware/",
    "local-assets/",
    "reference-assets/",
)


def tracked_files():
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    )
    return [ROOT / path.decode("utf-8") for path in result.stdout.split(b"\0") if path]


def allowlist_reason(relative: str) -> Optional[str]:
    reason = ALLOWLIST.get(relative)
    if reason is None:
        return None
    return reason.strip() or None


def is_blocked_tracked_path(relative: str) -> bool:
    return any(relative.startswith(prefix) for prefix in BLOCKED_TRACKED_PREFIXES)


def is_forbidden(path: Path) -> bool:
    relative = path.relative_to(ROOT).as_posix()
    if allowlist_reason(relative):
        return False
    return is_blocked_tracked_path(relative) or any(
        suffix.lower() in FORBIDDEN_SUFFIXES for suffix in path.suffixes
    )


def main() -> int:
    invalid_allowlist = [path for path, reason in ALLOWLIST.items() if not reason.strip()]
    if invalid_allowlist:
        print("Asset guard allowlist entries require non-empty reasons:")
        for path in invalid_allowlist:
            print(f" - {path}")
        return 1

    violations = [path.relative_to(ROOT).as_posix() for path in tracked_files() if is_forbidden(path)]

    if violations:
        print("Refusing forbidden tracked reference/media assets:")
        for violation in violations:
            print(f" - {violation}")
        print("If a generated rights-safe fixture is intentional, add its repo path to ALLOWLIST with a reason.")
        return 1

    print("Asset guard passed: no forbidden tracked reference/media assets found.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
