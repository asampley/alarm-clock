use core::hash::Hash;
use std::collections::HashSet;
use std::time;

use sysfs_gpio::{Direction, Pin};
use crate::note::Note;

pub struct Buzzer<N> where
	N: Note + Hash + Eq
{
	pin: Pin,
	notes: HashSet<N>,
	start: time::Instant,
}

impl<N> Buzzer<N> where
	N: Note + Hash + Eq
{
	pub fn new(pin_num: u64) -> sysfs_gpio::Result<Self> {
		let pin = Pin::new(pin_num);
		pin.export_direction(Direction::Out)?;

		Ok(Buzzer {
			pin: pin,
			notes: HashSet::default(),
			start: time::Instant::now(),
		})
	}

	pub fn add_note(&mut self, note: N) {
		self.notes.insert(note);
	}

	pub fn remove_note(&mut self, note: &N) {
		self.notes.remove(note);
	}

	pub fn clear(&mut self) {
		self.notes.clear();
	}

	pub fn update(&mut self) -> sysfs_gpio::Result<()> {
		let now = time::Instant::now();
		let elapsed = 1e-6 * (self.start.elapsed().as_micros() as f64);

		let val: isize = self.notes.iter()
			.map(|note| 1.0 / note.frequency())
			.map(|period| (elapsed % period) / period)
			.map(|n| if n >= 0.5 { 1 } else { -1 })
			.sum();

		self.pin.set_value(if val > 0 { 1 } else { 0 })
	}
}
