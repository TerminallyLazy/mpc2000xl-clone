# MPC2000XL Source Map

This file tracks evidence sources without copying proprietary source pages, scans, firmware, or media into the repository. Source IDs are stable references; local path hints are workstation pointers only.

## Source Categories

| Category | Use | Repository boundary |
| --- | --- | --- |
| manual | User-visible behavior, terminology, screen flow, and operating procedures. | Store only independently written notes plus page or section references. |
| schematic | Hardware signal, connector, board, and service-reference context. | Store only independently written notes plus sheet references. |
| firmware | User-supplied MPC2000XL OS image metadata, hashes, and observed runtime traces. | Never store firmware bytes or downloaded firmware artifacts. |
| vmpc | VMPC docs/source/license review findings used for comparative behavior only. | Store review notes and URLs, not copied third-party code or assets. |
| jjos | JJ-OS compatibility investigation boundary. | Treat as unverified for MPC2000XL until a real target is proven. |
| internal_spec | Repo-owned product, conformance, spike, and implementation specs. | Store normal repo documentation and tests. |

## Sources

| ID | Category | Type | Description | Local path hint or URL |
| --- | --- | --- | --- | --- |
| owner-manual | manual | owner_manual | MPC2000XL owner manual | `/Users/lazy/Downloads/akai_mpc2000xl_manual.pdf` |
| analog-schematic | schematic | service_schematic | MPC2000XL analog schematic | `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k analog.pdf` |
| main-schematic-1 | schematic | service_schematic | MPC2000XL main schematic, part 1 | `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k main 1_2.pdf` |
| main-schematic-2 | schematic | service_schematic | MPC2000XL main schematic, part 2 | `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k main 2_2.pdf` |
| operation-schematic | schematic | service_schematic | MPC2000XL operation schematic | `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k operation.pdf` |
| combined-schematic | schematic | service_schematic | MPC2000XL combined schematic | `/Users/lazy/Downloads/Akai-MPC-2000-XL-Schematic.pdf` |
| schematic-photo-1 | schematic | service_photo | MPC2000XL service manual schematic photo 1 | `/Users/lazy/Downloads/MPC2000XL_ServManual/P7200082.jpg` |
| schematic-photo-2 | schematic | service_photo | MPC2000XL service manual schematic photo 2 | `/Users/lazy/Downloads/MPC2000XL_ServManual/P7200083.jpg` |
| mpc2000xl-os-local-reference | firmware | user_supplied_os_image | Local Akai MPC2000XL OS image references for hash-only inspection and runtime traces | `/Users/lazy/firmware/mpc2000xl/` |
| firmware-spike-spec | internal_spec | spec | Firmware emulator spike design and asset boundary | `docs/superpowers/specs/2026-06-13-mpc2000xl-firmware-emulator-spike-design.md` |
| vmpc-docs-public | vmpc | public_docs | VMPC2000XL public documentation reference for comparative behavior research | `https://github.com/izzyreal/vmpc-docs` |
| vmpc-source-review | vmpc | source_review | VMPC source architecture/buildability review placeholder pending located source and license approval | `docs/superpowers/specs/2026-06-13-mpc2000xl-vmpc-fork-assessment-design.md` |
| vmpc-license-review | vmpc | license_review | VMPC license/dependency review placeholder before reuse or fork decisions | `docs/superpowers/specs/2026-06-13-mpc2000xl-vmpc-fork-assessment-design.md#license-and-asset-review` |
| jjos-unverified-boundary | jjos | investigation_boundary | JJ-OS support is not accepted as MPC2000XL evidence until a real MPC2000XL target is verified | `docs/superpowers/specs/2026-06-13-mpc2000xl-full-app-product-design.md#jj-os-evidence-boundary` |
| full-app-product-spec | internal_spec | spec | Full app product design notes | `docs/superpowers/specs/2026-06-13-mpc2000xl-full-app-product-design.md` |
| conformance-lab-spec | internal_spec | spec | Conformance lab design notes | `docs/superpowers/specs/2026-06-13-mpc2000xl-conformance-lab-design.md` |
| sequence-playback-scheduling-slice-plan | internal_spec | plan | Deterministic recorded sequence playback scheduling foundation | `docs/superpowers/plans/2026-06-13-mpc2000xl-sequence-playback-scheduling.md` |
| project-file-storage-slice-plan | internal_spec | plan | Rights-safe host-side JSON project file storage foundation | `docs/superpowers/plans/2026-06-13-mpc2000xl-project-file-storage.md` |
| midi-note-input-slice-plan | internal_spec | plan | Deterministic MIDI note-on/note-off input mapping foundation | `docs/superpowers/plans/2026-06-13-mpc2000xl-midi-note-input.md` |
| program-pad-parameter-editing-slice-plan | internal_spec | plan | PROGRAM pad Level/Pan/Tune editing and render propagation foundation | `docs/superpowers/plans/2026-06-13-mpc2000xl-program-pad-parameter-editing.md` |

## Legal Boundary

Do not copy proprietary manuals, firmware, service scans, hardware photos, copied artwork, logos, factory samples, third-party media, or audio/media samples into the repository.

## Mapping Rules

- Store independently written behavior summaries.
- Use source IDs and categories in behavior matrices, fixtures, tests, and implementation notes.
- Store page, section, and file references when known.
- Keep raw manuals, firmware, service scans, hardware photos, VMPC assets, copied source, and audio samples outside git.
- Treat local path hints as optional private lookup aids, not canonical source identity.
- Mark investigation gaps, unmapped streams, and conflicts between manual, VMPC, firmware, JJ-OS, schematic, and hardware traces explicitly.
