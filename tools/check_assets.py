#!/usr/bin/env python3
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN_SUFFIXES = {
    ".7z",
    ".aif",
    ".aiff",
    ".bin",
    ".bmp",
    ".dmg",
    ".flac",
    ".gif",
    ".img",
    ".iso",
    ".jpeg",
    ".jpg",
    ".m4a",
    ".mid",
    ".midi",
    ".mp3",
    ".ogg",
    ".pdf",
    ".png",
    ".rar",
    ".raw",
    ".rom",
    ".sit",
    ".snd",
    ".syx",
    ".tif",
    ".tiff",
    ".wav",
    ".webp",
    ".zip",
}
ALLOWLIST = {
    # Add generated, rights-safe assets here with a short justification in the commit.
}


def tracked_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files"],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    return [ROOT / line for line in result.stdout.splitlines() if line]


def is_forbidden(path: Path) -> bool:
    relative = path.relative_to(ROOT).as_posix()
    return relative not in ALLOWLIST and path.suffix.lower() in FORBIDDEN_SUFFIXES


def main() -> int:
    violations = [path.relative_to(ROOT).as_posix() for path in tracked_files() if is_forbidden(path)]

    if violations:
        print("Refusing forbidden tracked reference/media assets:")
        for violation in violations:
            print(f" - {violation}")
        print("If a generated rights-safe fixture is intentional, add its repo path to ALLOWLIST with a commit note.")
        return 1

    print("Asset guard passed: no forbidden tracked reference/media assets found.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
