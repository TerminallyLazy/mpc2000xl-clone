#!/usr/bin/env python3
from pathlib import Path
from pathlib import PurePosixPath
import subprocess
import sys
from dataclasses import dataclass
from typing import Optional

ROOT = Path(__file__).resolve().parents[1]
MAX_SAMPLE_BYTES = 8192
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
    (b"8BPS", "psd content"),
)
RIFF_BLOCKED_TYPES = {b"WAVE", b"AVI ", b"WEBP"}
FORM_BLOCKED_TYPES = {b"AIFF", b"AIFC"}
TEXT_BYTE_WHITELIST = set(range(32, 127)) | {9, 10, 13, 12, 8}
UTF8_TEXT_CONTROLS = {"\t", "\n", "\r", "\f", "\b"}


@dataclass(frozen=True)
class TrackedBlob:
    mode: str
    oid: str
    path: str


def tracked_blobs() -> list[TrackedBlob]:
    result = subprocess.run(
        ["git", "ls-files", "-s", "-z"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    )
    entries = []
    for raw_entry in result.stdout.split(b"\0"):
        if not raw_entry:
            continue
        metadata, raw_path = raw_entry.split(b"\t", 1)
        mode, oid, _stage = metadata.decode("ascii").split(" ", 2)
        entries.append(
            TrackedBlob(
                mode=mode,
                oid=oid,
                path=raw_path.decode("utf-8", errors="surrogateescape"),
            )
        )
    return entries


def allowlist_reason(relative: str) -> Optional[str]:
    reason = ALLOWLIST.get(relative)
    if not isinstance(reason, str):
        return None
    return reason.strip() or None


def is_blocked_tracked_path(relative: str) -> bool:
    return any(relative.startswith(prefix) for prefix in BLOCKED_TRACKED_PREFIXES)


def blob_sample(blob: TrackedBlob) -> bytes:
    result = subprocess.run(
        ["git", "cat-file", "-p", blob.oid],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    )
    return result.stdout[:MAX_SAMPLE_BYTES]


def textual_asset_reason(sample: bytes) -> Optional[str]:
    stripped = sample.lstrip()
    lowered = stripped[:512].lower()
    if lowered.startswith(b"<svg") or (lowered.startswith(b"<?xml") and b"<svg" in lowered):
        return "svg content"
    if stripped.startswith(b"%!PS-Adobe") or stripped.startswith(b"%!PS"):
        return "postscript content"
    return None


def container_reason(sample: bytes) -> Optional[str]:
    if sample.startswith(b"RIFF") and len(sample) >= 12 and sample[8:12] in RIFF_BLOCKED_TYPES:
        riff_type = sample[8:12].decode("ascii", errors="replace").strip()
        return f"riff {riff_type} content"
    if sample.startswith(b"FORM") and len(sample) >= 12 and sample[8:12] in FORM_BLOCKED_TYPES:
        form_type = sample[8:12].decode("ascii", errors="replace").strip()
        return f"form {form_type} content"
    if len(sample) >= 12 and sample[4:8] == b"ftyp":
        return "iso base media content"
    if len(sample) >= 262 and sample[257:262] == b"ustar":
        return "tar content"
    return None


def is_utf8_text(sample: bytes) -> bool:
    try:
        decoded = sample.decode("utf-8")
    except UnicodeDecodeError:
        return False
    return all(char >= " " or char in UTF8_TEXT_CONTROLS for char in decoded)


def binary_like_reason(sample: bytes) -> Optional[str]:
    if b"\0" in sample:
        return "binary content"

    if is_utf8_text(sample):
        return None

    try:
        sample.decode("utf-8")
    except UnicodeDecodeError:
        non_text = sum(byte not in TEXT_BYTE_WHITELIST for byte in sample)
        if len(sample) >= 128 and non_text / len(sample) > 0.30:
            return "binary-like content"
        return None

    return "binary control content"


def content_reason(blob: TrackedBlob) -> Optional[str]:
    if blob.mode == "160000":
        return None

    try:
        sample = blob_sample(blob)
    except subprocess.CalledProcessError as exc:
        return f"unreadable tracked blob {blob.oid}: git cat-file exited {exc.returncode}"

    if not sample:
        return None

    for prefix, reason in BLOCKED_MAGIC_PREFIXES:
        if sample.startswith(prefix):
            return reason

    return textual_asset_reason(sample) or container_reason(sample) or binary_like_reason(sample)


def violation_reason(blob: TrackedBlob) -> Optional[str]:
    if is_blocked_tracked_path(blob.path):
        return "tracked file under local research asset directory"
    if allowlist_reason(blob.path):
        return None
    for suffix in PurePosixPath(blob.path).suffixes:
        lowered = suffix.lower()
        if lowered in FORBIDDEN_SUFFIXES:
            return f"forbidden suffix {lowered}"
    return content_reason(blob)


def invalid_allowlist_entries(tracked_relatives: set[str]) -> list[tuple[str, str]]:
    invalid = []
    for path, reason in ALLOWLIST.items():
        reasons = []
        if not isinstance(reason, str):
            reasons.append("allowlist reason must be a string")
        elif not reason.strip():
            reasons.append("allowlist reason must be non-empty")
        if path not in tracked_relatives:
            reasons.append("allowlisted path is not tracked")
        if is_blocked_tracked_path(path):
            reasons.append("allowlisted path is under blocked local asset prefix")
        if reasons:
            invalid.append((path, "; ".join(reasons)))
    return invalid


def main() -> int:
    tracked = tracked_blobs()
    tracked_relatives = {blob.path for blob in tracked}
    invalid_allowlist = invalid_allowlist_entries(tracked_relatives)
    if invalid_allowlist:
        print("Asset guard allowlist entries are invalid:")
        for path, reason in invalid_allowlist:
            print(f" - {path}: {reason}")
        return 1

    violations = []
    for blob in tracked:
        reason = violation_reason(blob)
        if reason:
            violations.append((blob.path, reason))

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
