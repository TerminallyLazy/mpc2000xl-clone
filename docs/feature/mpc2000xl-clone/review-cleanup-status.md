# Review Cleanup Status

The generated desktop entrypoint and build-script source injection were removed after review. The desktop binary now uses the normal `apps/desktop/src/main.rs` entrypoint again.

The sample-flip planner remains in `mpc_core`. The desktop button should be added in a follow-up direct source edit to `apps/desktop/src/main.rs`, not through generated source replacement.
