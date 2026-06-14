# MPC2000XL DISK Mode Project Operations Foundation

## Behavior Slice

This slice turns DISK mode from a placeholder into a deterministic request screen for the existing host-side project JSON save/load flow. It is a machine-mode bridge to the current `mpc_storage` project-file functions, not native MPC2000XL disk support.

Implemented behavior:

- `DiskOperation` is a core enum with `save_project` and `load_project` JSON names.
- Machine state includes `selected_disk_operation`, defaulting to `SaveProject`.
- DISK LCD title is `DISK` and shows the selected operation, project-file JSON boundary, virtual host-path disk wording, and Save/Load soft-key labels.
- In DISK mode, cursor left/right toggles the selected operation.
- In DISK mode, non-zero data-wheel movement toggles the selected operation by delta sign; zero delta emits a structured ignore.
- Selection changes emit `DiskOperationSelected { operation }` plus `LcdChanged`.
- DISK F2 requests `SaveProject`; DISK F3 requests `LoadProject`.
- DISK F5/F6 remain unmapped in this slice; they do not request the selected operation.
- Non-DISK modes keep their existing soft-key behavior, including MAIN track/erase and PROGRAM assignment soft keys.
- Desktop handles `DiskOperationRequested(SaveProject)` by calling the existing `save_project_file()` function.
- Desktop handles `DiskOperationRequested(LoadProject)` by calling the existing `load_project_file()` function.
- Desktop continues to use the existing project file path and status controls; storage logic is not duplicated in DISK mode.

## Boundaries

- This is not Akai/MPC2000XL native disk format support.
- This does not parse, write, import, export, or claim compatibility with proprietary Akai disk, program, sample, or project formats.
- This does not add manuals, scans, firmware, images, samples, audio bytes, binary disk fixtures, or third-party code/assets.
- Behavior is internal-spec and fixture-backed until owner-manual, firmware, or hardware evidence is captured separately.

## Snapshot Policy

- `PROJECT_SNAPSHOT_VERSION` remains `1`.
- `machine.selected_disk_operation` is persisted in project snapshots.
- Current-reader backward compatibility is supported with a serde default: missing `selected_disk_operation` restores to `SaveProject`.
- Older-binary forward compatibility for this new v1 field is deferred to the formal project snapshot migration/versioning slice, matching the recent MIDI and sample navigation slices.
- Until that migration/versioning slice lands, foundation builds explicitly allow additive v1 JSON schema fields for repo-owned metadata snapshots. Version `1` is not yet a public stable file-format promise for older binaries.

## Verification Targets

- `cargo fmt --all --check`
- `cargo test -p mpc_core disk -- --nocapture`
- `cargo test -p mpc_conformance -- --nocapture`
- `cargo test --workspace`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `python3 -m json.tool docs/evidence/behavior-matrix.json >/dev/null`
- `git diff --check`

## Source And Evidence Status

- Status: fixture-backed internal spec.
- Source ID: `disk-mode-project-operations-slice-plan`.
- Reference gap: exact MPC2000XL DISK screen flow, native file naming, drive semantics, error UX, and binary format behavior remain unmapped.
