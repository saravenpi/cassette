#![forbid(clippy::unwrap_used)]
#![forbid(clippy::expect_used)]

use assert_no_alloc::assert_no_alloc;

pub struct AudioBuffer {
    samples: Vec<f32>,
    channels: usize,
    sample_rate: u32,
}

impl AudioBuffer {
    pub fn new(sample_rate: u32, channels: usize, frame_size: usize) -> Self {
        Self {
            samples: vec![0.0; frame_size * channels],
            channels,
            sample_rate,
        }
    }

    pub fn run_no_alloc<F, T>(f: F) -> T
    where
        F: FnOnce() -> T,
    {
        assert_no_alloc(f)
    }
}

pub trait AudioBackend {
    fn sample_rate(&self) -> u32;
    fn start(&mut self, callback: Box<dyn FnMut(&mut [f32]) + Send>);
    fn stop(&mut self);
}

#[cfg(feature = "cpal")]
mod cpal_backend {
    use cpal;

    pub struct CpalBackend {
        device: cpal::Device,
        config: cpal::StreamConfig,
        stream: Option<cpal::Stream>,
    }

    impl super::AudioBackend for CpalBackend {
        fn sample_rate(&self) -> u32 { self.config.sample_rate.0 }
        fn start(&mut self, callback: Box<dyn FnMut(&mut [f32]) + Send>) {
            let stream = match self.device.build_output_stream(
                &self.config, callback, |_| {}, None
            ) {
                Ok(s) => s,
                Err(_) => return,
            };
            let _ = stream.play();
            self.stream = Some(stream);
        }
        fn stop(&mut self) {
            if let Some(stream) = self.stream.take() {
                let _ = stream.pause();
            }
        }
    }
}

#[cfg(not(feature = "cpal"))]
mod cpal_backend {
    pub struct CpalBackend {
        dummy: bool,
    }

    impl super::AudioBackend for CpalBackend {
        fn sample_rate(&self) -> u32 { 44100 }
        fn start(&mut self, _callback: Box<dyn FnMut(&mut [f32]) + Send>) {}
        fn stop(&mut self) {}
    }
}

pub use cpal_backend::CpalBackend;

#[cfg(test)]
pub struct TestBackend {
    pub sample_rate: u32,
    pub running: bool,
}

#[cfg(test)]
impl AudioBackend for TestBackend {
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn start(&mut self, _callback: Box<dyn FnMut(&mut [f32]) + Send>) { self.running = true; }
    fn stop(&mut self) { self.running = false; }
}