# MPC2000XL SETUP Mode Preferences Foundation

## Behavior Slice

This slice turns SETUP mode from a placeholder into a deterministic internal preferences editor. It does not claim exact MPC2000XL setup-screen parity, JJ-OS parity, service-menu behavior, hardware LCD calibration, metronome audio scheduling, count-in transport behavior, or native project-file compatibility.

Implemented behavior:

- SETUP preferences are machine project metadata: `metronome_enabled`, `count_in_bars`, and `lcd_contrast`.
- Defaults are metronome enabled, count-in bars `0`, and LCD contrast `5`.
- Bounds are count-in bars `0..=4` and LCD contrast `0..=10`.
- SETUP mode LCD title is `SETUP` and shows selected-field markers plus the current metronome, count-in, and contrast values.
- Cursor left/right cycles `metronome`, `count_in_bars`, and `lcd_contrast`.
- Cursor field changes emit `SetupPreferencesChanged { preferences, selected_field }` plus `LcdChanged`.
- Data wheel edits the selected field:
  - metronome: positive delta enables, negative delta disables;
  - count-in bars: clamped to `0..=4`;
  - LCD contrast: clamped to `0..=10`.
- Zero-delta edits emit deterministic `Ignored` reasons under `setup.<field>.zero_delta_ignored`.
- Edits that would not change a value at a boundary emit deterministic `Ignored` reasons under `setup.<field>.boundary`.
- SETUP soft keys remain unmapped and emit `Ignored { reason: "setup.soft_key.<index>_unmapped" }`.
- Desktop status surfaces current SETUP preferences and selected field.

## Snapshot Policy

- Project snapshots persist a top-level `setup` object with `preferences` and `selected_field`.
- Missing top-level `setup` restores documented defaults for older snapshots.
- Present incomplete `setup` objects are rejected during JSON prevalidation.
- Present incomplete `setup.preferences` objects are rejected during JSON prevalidation.
- Structured restore and JSON restore both validate count-in and LCD contrast bounds.
- Snapshot compatibility policy for this slice is current-reader backward compatibility only; older-binary forward compatibility remains deferred to a formal migration/versioning slice.

## Source And Evidence Status

- Status: fixture-backed internal spec.
- Rights boundary: no manuals, service scans, firmware bytes, JJ-OS assets, hardware captures, copied third-party code, audio bytes, factory samples, or proprietary setup-screen assets are stored in the repo.
- Reference gap: exact MPC2000XL SETUP pages, metronome/count-in behavior, LCD contrast behavior, and JJ-OS setup behavior remain unmapped.
- This foundation exists to make SETUP state deterministic, visible, serializable, and testable before external behavior evidence is mapped.

## Verification Targets

- `cargo fmt --all --check`
- `cargo test -p mpc_core setup -- --test-threads=1`
- `cargo test -p mpc_conformance --test fixtures all_json_fixtures_with_source_references_pass -- --exact --test-threads=1`
- `cargo test --workspace -- --test-threads=1`
- `CARGO_TARGET_DIR=/tmp/mpc2000xl-setup-desktop-check-target cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `python3 -m json.tool docs/evidence/behavior-matrix.json >/dev/null`
- `git diff --check`
