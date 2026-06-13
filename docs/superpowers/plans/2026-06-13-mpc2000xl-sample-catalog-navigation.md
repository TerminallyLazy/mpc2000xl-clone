# MPC2000XL Sample Catalog Navigation Foundation

## Slice Goal

Add a deterministic SAMPLE/TRIM catalog navigation foundation using only generated program sample metadata. This slice does not import, decode, store, or trim real audio bytes.

## Behavior Slice

- `Program01` derives a sorted unique sample catalog from current pad assignments.
- Catalog identity is the sample id. Duplicate sample ids collapse to the first sorted assignment so persisted `selected_sample_id` can always resolve to a single catalog row.
- Default selection resolves to the first catalog entry, `synthetic_a_01` / `SYN-A01`.
- SAMPLE and TRIM share one selected sample identity.
- SAMPLE shows selected index/count, sample id/name, source pad, deterministic metadata length, and a metadata-only/no-audio-bytes notice.
- TRIM shows the same selected sample plus deterministic placeholder trim metadata: start frame, end frame, length frames, and source pad.
- Data-wheel movement in SAMPLE/TRIM clamps to catalog bounds and emits `SampleSelected` plus `LcdChanged`.
- Empty catalogs are valid metadata states after restore; navigation returns `Ignored { reason: "sample_catalog.empty" }` without panic.
- Project snapshots persist `machine.selected_sample_id` with `serde(default)` compatibility. Missing or stale selected sample ids normalize to the first available sample on restore, or `None` if the catalog is empty.
- Snapshot compatibility policy for this foundation line is current-reader backward compatibility through defaults and normalization. Older binaries are not guaranteed to forward-read newer foundation fields until a formal project format versioning slice is added.

## Source And Evidence Status

- Status: fixture-backed internal spec.
- Rights boundary: metadata only; no WAV data, factory samples, firmware bytes, manuals, service scans, or third-party media are stored in the repo.
- Reference gap: exact MPC2000XL/JJ-OS sample record, sample list, and trim edit behavior remains unmapped.
- Current implementation derives placeholder frame lengths from the source pad address for deterministic testability only.
- `PROJECT_SNAPSHOT_VERSION` remains the foundation version while schema migration policy is still internal; forward compatibility with older binaries is a deferred project-format concern.

## Verification Targets

- `cargo test -p mpc_core sample_catalog -- --nocapture`
- `cargo test -p mpc_conformance`
- `cargo test --workspace`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `git diff --check`
