use serde::Deserialize;

use crate::synth::SynthConfig;

#[derive(Deserialize)]
pub struct Config {
	pub button_bounce_ms: u64,

	pub brightness: u8,
	pub scroll_delay_ms: u64,
	pub ascii_uppercase: bool,

	pub synth_config: SynthConfig,
}
