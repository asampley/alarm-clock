use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
	pub buzzer_pin: u8,
	pub button_pins: Vec<u8>,
	pub midi_dir: String,
}
