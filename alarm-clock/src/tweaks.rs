use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
	pub button_bounce_ms: u64,

	pub brightness: u8,
	pub scroll_delay_ms: u64,
	pub ascii_uppercase: bool,

	pub synth_sustain_ratio: f64,
	pub synth_decay_constant: f64,
	pub synth_release_decay_constant: f64
}
