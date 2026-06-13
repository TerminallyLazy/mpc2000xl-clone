# MPC2000XL Clone

This repository is the foundation for a full cross-platform desktop instrument modeled on the MPC2000XL workflow.

The product target is a full app, not a reduced prototype. The first code layers establish deterministic core behavior, fixture-backed conformance, a rights-safe desktop shell, and research tooling for user-supplied firmware images.

## Legal Asset Boundary

Do not commit proprietary Akai OS images, manuals, service scans, factory samples, copied front-panel artwork, logos, or hardware photos. Research docs can reference local paths and public URLs. Code and tests must use independently written behavior descriptions, synthetic fixtures, clean metadata, and user-supplied assets.

## Verification

The full verification entrypoint will be added by the foundation implementation plan as `./scripts/verify.sh`.

At the workspace-skeleton stage, this command is expected to fail until the workspace member crates are added:

```bash
cargo metadata --no-deps
```
