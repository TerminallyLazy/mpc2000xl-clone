# MPC2000XL VMPC Fork Assessment Design

Date: 2026-06-13
Status: Draft for user review
Related product spec: `docs/superpowers/specs/2026-06-13-mpc2000xl-full-app-product-design.md`

## Purpose

Assess whether VMPC2000XL can accelerate the full desktop app without compromising fidelity, licensing, architecture, or product control. This is an assessment spec, not a commitment to fork.

## Inputs

- VMPC2000XL documentation repository: `https://github.com/izzyreal/vmpc-docs`
- VMPC2000XL public documentation, including its claim of standalone desktop and plugin formats for Linux, macOS, iPadOS, and Windows.
- VMPC source repository or release artifacts, once located and reviewed.
- Local MPC2000XL manual and service/schematic resources.
- The conformance behavior matrix.

## Assessment Questions

1. What license governs VMPC code, assets, docs, and dependencies?
2. Is the source available, buildable, and actively maintainable enough to use?
3. Which platforms and package types does it already support?
4. Does its architecture separate machine behavior from UI, audio/MIDI, storage, and host shell?
5. Which MPC2000XL workflows are already implemented, partially implemented, or missing?
6. Can VMPC behavior be tested through an automated harness?
7. Does it use original assets or third-party material that would conflict with this product's legal boundary?
8. Is upstream collaboration practical, or would a fork be necessary?

## Review Areas

### License And Rights

Review code license, asset license, dependency licenses, documentation license, and contribution terms. Identify obligations, incompatibilities, attribution requirements, and any assets that cannot be reused.

### Build And Packaging

Build from source on macOS first, then inspect Windows and Linux feasibility. Record toolchain versions, dependency problems, packaging status, and reproducibility.

### Architecture

Map VMPC into the target app boundaries:

- Machine core.
- Front panel runtime.
- Audio/MIDI engine.
- Storage/file engine.
- Desktop shell.
- Tests/conformance.

If boundaries are tangled, document what would need to be extracted or wrapped.

### Behavior Coverage

Compare VMPC behavior against the behavior matrix. Track:

- Implemented workflows.
- Missing workflows.
- Known deviations.
- Untested assumptions.
- Areas where VMPC can serve as comparative evidence.

### Integration Options

Possible outcomes:

- Use VMPC as a reference only.
- Reuse specific components if licensing and architecture permit.
- Fork and harden VMPC into the full app.
- Contribute upstream while maintaining separate product-specific packaging.
- Do not use VMPC because of license, architecture, behavior, or maintainability blockers.

## Decision Criteria

VMPC is a good accelerator if:

- License terms are compatible.
- Source builds reliably.
- Core behavior can be tested or extracted.
- Major MPC workflows already exist.
- Asset rights are clean or replaceable.
- Gaps are smaller than a clean-room rewrite.

VMPC is not a good accelerator if:

- License or asset rights are incompatible.
- Source is unavailable or not practically buildable.
- Machine behavior is too coupled to UI or host shell.
- Behavior coverage is too shallow.
- The fork would make full-app conformance harder than a native core.

## Assessment Deliverables

- License summary.
- Build report.
- Architecture map.
- Behavior coverage table.
- Gap list.
- Integration options.
- Recommendation with rationale.
- Follow-up implementation plan only after user approval and writing-plans transition.

## Risks

- Documentation may be easier to access than the actual implementation.
- Existing UI fidelity may mask missing machine behavior.
- Forking can create long-term maintenance cost.
- Upstream collaboration can be slower than direct product implementation.
- Reused assets can violate the product legal boundary if not audited.
