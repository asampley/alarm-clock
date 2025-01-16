#[pre_table::freq_table]
const F: [f64; 256];

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct MidiNote(pub i8);

impl MidiNote {
	pub const fn frequency(&self) -> f64 {
		F[(self.0 as u8) as usize]
	}
}
