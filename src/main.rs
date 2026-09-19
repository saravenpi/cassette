//! Binary entry point.
//!
//! Everything lives in the library so it can be tested without a terminal.

// The red-zone allocator only exists in debug builds: `assert_no_alloc` compiles
// its checks out in release, so the type is not even exported there.
#[cfg(debug_assertions)]
#[global_allocator]
static GLOBAL_ALLOC: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

fn main() -> anyhow::Result<()> {
    let headless = std::env::args().any(|arg| arg == "--headless" || arg == "--plain");
    if headless {
        terminal_daw::app::App::run_headless()
    } else {
        terminal_daw::app::App::run()
    }
}
