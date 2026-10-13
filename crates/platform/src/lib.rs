//! SDL3 platform layer: window, input, audio output, timing.
//!
//! Only this crate and `apps/*` may depend on SDL3 (see scripts/check-deps.sh).

/// Version of the SDL3 library linked into the binary.
pub fn sdl_version() -> String {
    sdl3::version::version().to_string()
}
