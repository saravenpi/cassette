const MAX_CHANNELS: usize = 32;

pub struct Mixer {
    nodes: [Option<Box<dyn AudioNode>>; MAX_CHANNELS],
}

pub trait AudioNode {
    fn process(&mut self) -> (f32, f32);
}

impl Default for Mixer {
    fn default() -> Self {
        Self {
            nodes: [const { None }; MAX_CHANNELS],
        }
    }
}

impl Mixer {
    pub fn new() -> Self { Self::default() }

    pub fn add_node(&mut self, node: Box<dyn AudioNode>) -> Option<usize> {
        for (i, slot) in self.nodes.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(node);
                return Some(i);
            }
        }
        None
    }

    pub fn mix(&mut self, output: &mut [f32]) {
        for frame in 0..output.len() / 2 {
            let (l, r) = self.process_frame();
            output[frame * 2] = l;
            output[frame * 2 + 1] = r;
        }
    }

    fn process_frame(&mut self) -> (f32, f32) {
        let mut sum_l = 0.0f32;
        let mut sum_r = 0.0f32;
        for slot in &mut self.nodes {
            if let Some(node) = slot {
                let (l, r) = node.process();
                sum_l += l;
                sum_r += r;
            }
        }
        (sum_l.clamp(-1.0, 1.0), sum_r.clamp(-1.0, 1.0))
    }
}