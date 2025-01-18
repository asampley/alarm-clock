use midly::num::u7;

#[pre_table::freq_table]
static F: [f64; 128];

#[derive(Debug)]
pub struct MidiNote {
	pub key: u7,
	pub vel: u7,
}

impl MidiNote {
	pub fn frequency(&self) -> f64 {
		F[(self.key.as_int()) as usize]
	}

	pub fn on(&self) -> bool {
		self.vel > 0
	}
}
