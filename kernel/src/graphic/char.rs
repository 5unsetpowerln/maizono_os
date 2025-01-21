#[derive(Debug, Copy, Clone)]
pub struct AsciiChar {
    code: u8,
}

impl AsciiChar {
    pub const fn new(code: u8) -> Self {
        Self { code }
    }
}
