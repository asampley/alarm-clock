use crate::pre_table::freq_table;

freq_table!(F);

pub trait Note {
	const A0: f64 = 27.5;

	fn frequency(&self) -> f64;
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct MidiNote(pub i8);

impl Note for MidiNote {
	fn frequency(&self) -> f64 {
		F[(self.0 as u8) as usize]
	}
}
