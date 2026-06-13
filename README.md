# MPC2000XL Clone

This repository is the foundation for a full cross-platform desktop instrument modeled on the MPC2000XL workflow.

The product target is a full app, not a reduced prototype. The first code layers establish deterministic core behavior, fixture-backed conformance, a rights-safe desktop shell, and research tooling for user-supplied firmware images.

## Legal Asset Boundary

Do not commit proprietary Akai OS images, manuals, service scans, factory samples, copied front-panel artwork, logos, or hardware photos. Research docs can reference local paths and public URLs. Code and tests must use independently written behavior descriptions, synthetic fixtures, clean metadata, and user-supplied assets.

## Verification

Run the workspace checks from the repository root:

```bash
cargo fmt --all --check
cargo test --workspace
cargo check -p mpc_desktop
python3 tools/check_assets.py
```

## Project Files

The desktop shell can save and load repo-owned metadata snapshots at paths ending in `.mpc2000xl-project.json`. The default path is ignored by git:

```bash
local-assets/projects/last.mpc2000xl-project.json
```

These files are JSON produced by `mpc_core` project snapshot APIs. They do not embed audio bytes or proprietary Akai assets, and they are not a claim of native MPC2000XL disk-format compatibility.
