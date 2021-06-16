use super::gpio::{Gpio, GpioError};
use core::hash::Hash;
use std::collections::HashMap;
use std::time;

use sysfs_gpio::{Direction, Pin};
use crate::note::Note;

pub struct Buzzer<N> where
	N: Note + Hash + Eq
{
	pin: Pin,
	notes: HashMap<N, time::Instant>,
}

impl<N> Buzzer<N> where
	N: Note + Hash + Eq
{
	pub fn new(pin_num: u64) -> Result<Self, GpioError> {
		let pin = Pin::new(pin_num);
		pin.export_direction(Direction::Out)?;

		Ok(Buzzer {
			pin: pin,
			notes: HashMap::default(),
		})
	}

	pub fn add_note(&mut self, note: N) {
		self.notes.insert(note, time::Instant::now());
	}

	pub fn remove_note(&mut self, note: &N) {
		self.notes.remove(note);
	}

	pub fn clear(&mut self) {
		self.notes.clear();
	}

	pub fn update(&mut self) -> sysfs_gpio::Result<()> {
		let now = time::Instant::now();

		for (note, since_play) in &mut self.notes {
			let t = *since_play + time::Duration::from_micros((1_000_000.0 / note.frequency()) as u64);

			if t < now {
				*since_play = t;
				self.pin.set_value(1)?;
			}
		}

		self.pin.set_value(0)?;

		Ok(())
	}
}
