mod app;
mod core;
mod events;
mod input;
mod models;
mod ui;
mod utils;

use app::App;

#[global_allocator]
static GLOBAL_ALLOC: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

fn main() {
    let mut app = App::init();
    app.run();
}
