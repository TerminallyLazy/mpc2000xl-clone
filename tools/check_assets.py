#!/usr/bin/env python3
from pathlib import Path
import subprocess
import sys

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


def tracked_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    )
    return [ROOT / path.decode("utf-8") for path in result.stdout.split(b"\0") if path]


def is_forbidden(path: Path) -> bool:
    relative = path.relative_to(ROOT).as_posix()
    return relative not in ALLOWLIST and any(
        suffix.lower() in FORBIDDEN_SUFFIXES for suffix in path.suffixes
    )


def main() -> int:
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
