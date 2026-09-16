pub struct Track {
    pub name: String,
    pub muted: bool,
    pub output_node: Option<usize>,
}

impl Track {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            muted: false,
            output_node: None,
        }
    }
}