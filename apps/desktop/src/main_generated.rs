// The generated file is built from `src/main.rs` by `build.rs` so this stacked PR
// can wire the desktop sample-flip UI without duplicating the current desktop shell.
include!(concat!(env!("OUT_DIR"), "/main_with_sample_flip_ui.rs"));
