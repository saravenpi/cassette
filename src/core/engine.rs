use std::sync::atomic::Ordering;
use crate::core::transport::Transport;
use crate::events::bus::EventBus;
use crate::events::types::Event;

pub struct Engine {
    transport: Transport,
    bus: EventBus,
}

impl Engine {
    pub fn new(bus: EventBus, sample_rate: u32, bpm: f32) -> Self {
        Self {
            transport: Transport::new(sample_rate, bpm),
            bus,
        }
    }

    pub fn audio_tick(&self) {
        if self.transport.advance_frame().is_some() {
            self.bus.send(Event::TransportTick);
        }
    }

    pub fn play(&mut self) {
        self.transport.playing = true;
        let frame_offset = self.transport.position.load(Ordering::Relaxed);
        self.bus.send(Event::Play { frame_offset });
    }

    pub fn stop(&mut self) {
        self.transport.playing = false;
        let frame_offset = self.transport.position.load(Ordering::Relaxed);
        self.transport.position.store(0, Ordering::Relaxed);
        self.bus.send(Event::Stop { frame_offset });
    }

    pub fn record(&mut self) {
        let frame_offset = self.transport.position.load(Ordering::Relaxed);
        self.bus.send(Event::Record { frame_offset });
    }

    pub fn position_frames(&self) -> u64 {
        self.transport.position_frames()
    }
}