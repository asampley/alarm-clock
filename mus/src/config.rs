use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
	pub buzzer_pin: u64,
	pub button_pins: Vec<u64>,
	pub midi_dir: String,
}
