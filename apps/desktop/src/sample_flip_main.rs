// The build script writes a generated desktop entrypoint that starts from the
// canonical `src/main.rs` implementation and injects the rights-safe sample flip
// controls from the stacked planner PR.
include!(concat!(env!("OUT_DIR"), "/desktop_sample_flip_main.rs"));
