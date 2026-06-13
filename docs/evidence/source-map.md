# MPC2000XL Source Map

This file tracks evidence sources without copying proprietary source pages, scans, firmware, or media into the repository. Source IDs are stable references; local path hints are workstation pointers only.

## Sources

| ID | Type | Description | Local path hint |
| --- | --- | --- | --- |
| owner-manual | owner_manual | MPC2000XL owner manual | `/Users/lazy/Downloads/akai_mpc2000xl_manual.pdf` |
| analog-schematic | service_schematic | MPC2000XL analog schematic | `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k analog.pdf` |
| main-schematic-1 | service_schematic | MPC2000XL main schematic, part 1 | `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k main 1_2.pdf` |
| main-schematic-2 | service_schematic | MPC2000XL main schematic, part 2 | `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k main 2_2.pdf` |
| operation-schematic | service_schematic | MPC2000XL operation schematic | `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k operation.pdf` |
| combined-schematic | service_schematic | MPC2000XL combined schematic | `/Users/lazy/Downloads/Akai-MPC-2000-XL-Schematic.pdf` |
| schematic-photo-1 | service_photo | MPC2000XL service manual schematic photo 1 | `/Users/lazy/Downloads/MPC2000XL_ServManual/P7200082.jpg` |
| schematic-photo-2 | service_photo | MPC2000XL service manual schematic photo 2 | `/Users/lazy/Downloads/MPC2000XL_ServManual/P7200083.jpg` |
| full-app-product-spec | spec | Full app product design notes | `docs/superpowers/specs/2026-06-13-mpc2000xl-full-app-product-design.md` |
| conformance-lab-spec | spec | Conformance lab design notes | `docs/superpowers/specs/2026-06-13-mpc2000xl-conformance-lab-design.md` |

## Legal Boundary

Do not copy proprietary manuals, firmware, service scans, hardware photos, or audio/media samples into the repository.

## Mapping Rules

- Store independently written behavior summaries.
- Use source IDs in behavior matrices, fixtures, tests, and implementation notes.
- Store page, section, and file references when known.
- Keep raw manuals, firmware, service scans, hardware photos, and audio samples outside git.
- Treat local path hints as optional private lookup aids, not canonical source identity.
- Mark conflicts between manual, VMPC, firmware, and hardware traces explicitly.
