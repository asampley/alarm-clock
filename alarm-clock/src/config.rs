use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
	button_bounce_ms: u64,
	pub brightness: u8,
	pub scroll_delay_ms: u64,
	pub ascii_uppercase: bool,
}

impl Config {
	pub fn button_bounce_ms(&self) -> u64 {
		self.button_bounce_ms
	}
}
