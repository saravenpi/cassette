use crate::events::bus::EventBus;

pub struct Renderer {
    bus: EventBus,
}

impl Renderer {
    pub fn new(bus: EventBus) -> Self {
        Self { bus }
    }

    pub fn draw(&mut self) {}
}
