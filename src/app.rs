use crate::events::bus::EventBus;

pub struct App {
    bus: EventBus,
    running: bool,
}

impl App {
    pub fn init() -> Self {
        let bus = EventBus::new();
        Self { bus, running: true }
    }

    pub fn run(&mut self) {
        while self.running {
            self.update();
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }

    fn update(&mut self) {
        std::thread::yield_now();
    }
}
