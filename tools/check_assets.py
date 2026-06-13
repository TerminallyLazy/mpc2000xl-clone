#!/usr/bin/env python3
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN_SUFFIXES = {
    ".aif",
    ".aiff",
    ".bin",
    ".img",
    ".iso",
    ".jpeg",
    ".jpg",
    ".pdf",
    ".png",
    ".rom",
    ".snd",
    ".wav",
}
SKIP_DIRS = {".git", ".superpowers", "target"}


def is_skipped(path: Path) -> bool:
    return any(part in SKIP_DIRS for part in path.relative_to(ROOT).parts)


def main() -> int:
    violations: list[str] = []
    for path in ROOT.rglob("*"):
        if not path.is_file() or is_skipped(path):
            continue
        if path.suffix.lower() in FORBIDDEN_SUFFIXES:
            violations.append(str(path.relative_to(ROOT)))

    if violations:
        print("Refusing proprietary or binary reference assets in git:")
        for violation in violations:
            print(f" - {violation}")
        return 1

    print("Asset guard passed: no forbidden reference assets found.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
