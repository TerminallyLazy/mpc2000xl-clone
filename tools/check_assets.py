#!/usr/bin/env python3
from pathlib import Path
import subprocess
import sys
from typing import Optional

ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN_SUFFIXES = {
    ".7z",
    ".aac",
    ".ai",
    ".aif",
    ".aifc",
    ".aiff",
    ".avi",
    ".bin",
    ".bmp",
    ".bz2",
    ".dmg",
    ".eps",
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
    ".mpeg",
    ".mp3",
    ".mp4",
    ".mpg",
    ".ogg",
    ".opus",
    ".pdf",
    ".png",
    ".psd",
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
    ".webm",
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
BLOCKED_MAGIC_PREFIXES = (
    (b"%PDF-", "pdf content"),
    (b"\x89PNG\r\n\x1a\n", "png content"),
    (b"\xff\xd8\xff", "jpeg content"),
    (b"GIF87a", "gif content"),
    (b"GIF89a", "gif content"),
    (b"BM", "bmp content"),
    (b"fLaC", "flac content"),
    (b"OggS", "ogg content"),
    (b"ID3", "mp3 content"),
    (b"MThd", "midi content"),
    (b"Rar!", "rar content"),
    (b"7z\xbc\xaf\x27\x1c", "7z content"),
    (b"\x1f\x8b", "gzip content"),
    (b"BZh", "bzip2 content"),
    (b"\xfd7zXZ\x00", "xz content"),
    (b"PK\x03\x04", "zip content"),
)
RIFF_BLOCKED_TYPES = {b"WAVE", b"AVI ", b"WEBP"}
TEXT_BYTE_WHITELIST = set(range(32, 127)) | {9, 10, 13, 12, 8}


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
    if not isinstance(reason, str):
        return None
    return reason.strip() or None


def is_blocked_tracked_path(relative: str) -> bool:
    return any(relative.startswith(prefix) for prefix in BLOCKED_TRACKED_PREFIXES)


def content_reason(path: Path) -> Optional[str]:
    try:
        with path.open("rb") as file:
            sample = file.read(8192)
    except OSError as exc:
        return f"unreadable tracked file: {exc}"

    if not sample:
        return None

    for prefix, reason in BLOCKED_MAGIC_PREFIXES:
        if sample.startswith(prefix):
            return reason

    if sample.startswith(b"RIFF") and len(sample) >= 12 and sample[8:12] in RIFF_BLOCKED_TYPES:
        riff_type = sample[8:12].decode("ascii", errors="replace").strip()
        return f"riff {riff_type} content"

    if b"\0" in sample:
        return "binary content"

    non_text = sum(byte not in TEXT_BYTE_WHITELIST for byte in sample)
    if len(sample) >= 128 and non_text / len(sample) > 0.30:
        return "binary-like content"

    return None


def violation_reason(path: Path) -> Optional[str]:
    relative = path.relative_to(ROOT).as_posix()
    if is_blocked_tracked_path(relative):
        return "tracked file under local research asset directory"
    if allowlist_reason(relative):
        return None
    for suffix in path.suffixes:
        lowered = suffix.lower()
        if lowered in FORBIDDEN_SUFFIXES:
            return f"forbidden suffix {lowered}"
    return content_reason(path)


def main() -> int:
    tracked = tracked_files()
    tracked_relatives = {path.relative_to(ROOT).as_posix() for path in tracked}
    invalid_allowlist = [
        path for path, reason in ALLOWLIST.items()
        if not isinstance(reason, str) or not reason.strip() or path not in tracked_relatives
    ]
    if invalid_allowlist:
        print("Asset guard allowlist entries require non-empty reasons and tracked paths:")
        for path in invalid_allowlist:
            print(f" - {path}")
        return 1

    violations = []
    for path in tracked:
        reason = violation_reason(path)
        if reason:
            violations.append((path.relative_to(ROOT).as_posix(), reason))

    if violations:
        print("Refusing forbidden tracked reference/media assets:")
        for violation, reason in violations:
            print(f" - {violation}: {reason}")
        print("If a generated rights-safe fixture is intentional, add its repo path to ALLOWLIST with a reason.")
        print("Tracked files under local research asset directories are never allowlisted; keep them untracked.")
        return 1

    print("Asset guard passed: no forbidden tracked reference/media assets found.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
