pub mod error;
pub mod font;
pub mod framebuffer;

pub struct RgbColor {
    red: u8,
    green: u8,
    blue: u8,
}

impl RgbColor {
    const MAX_U32: u32 = 0xffffff;

    pub fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub fn with_intensity(self, intensity: u8) -> Self {
        let current_intensity =
            (299 * self.red as u32 + 587 * self.green as u32 + 114 * self.blue as u32) / 1000;

        if current_intensity == 0 {
            return Self {
                red: 0,
                green: 0,
                blue: 0,
            };
        }

        let ratio = (intensity as u32 * 1024) / current_intensity;

        let new_red = ((self.red as u32 * ratio) / 1024).min(0xff) as u8;
        let new_green = ((self.green as u32 * ratio) / 1024).min(0xff) as u8;
        let new_blue = ((self.blue as u32 * ratio) / 1024).min(0xff) as u8;

        return Self {
            red: new_red,
            green: new_green,
            blue: new_blue,
        };
    }
}
