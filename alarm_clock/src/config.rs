use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
	buzzer_pin: u8,
	button_pins: Vec<u8>,
	midi_dir: String,
	button_bounce_ms: u64,
	pub brightness: u8,
	pub scroll_delay_ms: u64,
	pub ascii_uppercase: bool,
	pub text_offset: i8,
}

impl Config {
	pub fn buzzer_pin(&self) -> u8 {
		self.buzzer_pin
	}

	pub fn button_pins(&self) -> &[u8] {
		&self.button_pins
	}

	pub fn midi_dir(&self) -> &str {
		&self.midi_dir
	}

	pub fn button_bounce_ms(&self) -> u64 {
		self.button_bounce_ms
	}
}
