use core::hash::Hash;
use std::collections::HashMap;
use std::time;

use rppal::gpio::{ Gpio, OutputPin };
use crate::note::Note;

pub struct Buzzer<N> where
	N: Note + Hash + Eq
{
	pin: OutputPin,
	notes: HashMap<N, time::Instant>,
}

impl<N> Buzzer<N> where
	N: Note + Hash + Eq
{
	pub fn new(pin_num: u8) -> rppal::gpio::Result<Self> {
		let pin = Gpio::new()?.get(pin_num)?.into_output();

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

	pub fn update(&mut self) {
		let now = time::Instant::now();

		let mut play = false;

		for (note, since_play) in &mut self.notes {
			let t = *since_play + time::Duration::from_micros((1_000_000.0 / note.frequency()) as u64);

			if t < now {
				*since_play = t;
				play = true;
			}
		}

		if play {
			self.pin.set_high();
			std::thread::sleep(std::time::Duration::from_nanos(1));
			self.pin.set_low();
		}
	}
}
