use crate::{note::MidiNote, synth::Synth};

use embassy_time::Timer;
use embedded_hal::digital::OutputPin;
pub struct Buzzer<Pin, const SIZE: usize>
where
	Pin: OutputPin,
{
	pin: Pin,
	synth: Synth<SIZE>,
}

impl<Pin, const SIZE: usize> Buzzer<Pin, SIZE>
where
	Pin: OutputPin,
{
	pub fn new(pin: Pin, synth: Synth<SIZE>) -> Self {
		Self { pin, synth }
	}

	pub fn is_empty(&self) -> bool {
		self.synth.is_empty()
	}

	pub fn add_note(&mut self, note: MidiNote) -> Result<(), ()> {
		self.synth.add_note(note)
	}

	pub fn remove_note(&mut self, note: &MidiNote) {
		self.synth.release_note(note);
	}

	pub fn clear(&mut self) {
		self.synth.stop();
	}

	pub async fn update(&mut self) -> Result<(), Pin::Error> {
		if let Some(pulse) = self.synth.update() {
			self.pin.set_high()?;
			embassy_time::block_for(pulse.on);
			self.pin.set_low()?;
			Timer::after(pulse.off).await;
		}

		Ok(())
	}
}
