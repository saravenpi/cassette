//! Binary entry point.
//!
//! Everything lives in the library so it can be tested without a terminal.
//!
//! Usage:
//!   zboub69                      full-screen TUI, best audio device
//!   zboub69 --stream-port 9999   TUI + raw PCM stream on loopback TCP (SSH)
//!   zboub69 --headless           print one plain-text frame and exit

// The red-zone allocator only exists in debug builds: `assert_no_alloc` compiles
// its checks out in release, so the type is not even exported there.
#[cfg(debug_assertions)]
#[global_allocator]
static GLOBAL_ALLOC: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args
        .iter()
        .any(|arg| arg == "--headless" || arg == "--plain")
    {
        return terminal_daw::app::App::run_headless();
    }

    match stream_port(&args) {
        Ok(Some(port)) => terminal_daw::app::App::run_streaming(port),
        Ok(None) => terminal_daw::app::App::run(),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    }
}

/// Resolve the stream port from `--stream-port <n>` or `ZBOUB_STREAM_PORT`.
fn stream_port(args: &[String]) -> Result<Option<u16>, String> {
    if let Some(index) = args.iter().position(|arg| arg == "--stream-port") {
        let value = args
            .get(index + 1)
            .ok_or_else(|| "zboub69: --stream-port needs a port number".to_string())?;
        return value
            .parse()
            .map(Some)
            .map_err(|_| format!("zboub69: '{value}' is not a valid port"));
    }
    match std::env::var("ZBOUB_STREAM_PORT") {
        Ok(value) => value
            .parse()
            .map(Some)
            .map_err(|_| format!("zboub69: ZBOUB_STREAM_PORT='{value}' is not a valid port")),
        Err(_) => Ok(None),
    }
}
