use heapless::FnvIndexMap;

use crate::note::MidiNote;

use embassy_time::{Duration, Instant, Timer};
use embedded_hal::digital::OutputPin;

/// Chosen because humans should be able to hear at most a 19kHz, or 1/52us
///
/// Too small a delay reduces volume drastically
const NOTE_HALF_DELAY: Duration = Duration::from_micros(20);

pub struct Buzzer<Pin, const SIZE: usize>
where
	Pin: OutputPin,
{
	pin: Pin,
	notes: FnvIndexMap<MidiNote, Instant, SIZE>,
}

impl<Pin, const SIZE: usize> Buzzer<Pin, SIZE>
where
	Pin: OutputPin,
{
	pub fn new(pin: Pin) -> Self {
		Self {
			pin,
			notes: Default::default(),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.notes.is_empty()
	}

	pub fn add_note(&mut self, note: MidiNote) -> Result<Option<Instant>, (MidiNote, Instant)> {
		self.notes.insert(note, Instant::now())
	}

	pub fn remove_note(&mut self, note: &MidiNote) {
		self.notes.remove(note);
	}

	pub fn clear(&mut self) {
		self.notes.clear();
	}

	pub async fn update(&mut self) -> Result<(), Pin::Error> {
		let now = Instant::now();

		let mut play = false;

		for (note, since_play) in &mut self.notes {
			let t = *since_play + Duration::from_micros((1_000_000.0 / note.frequency()) as u64);

			if t < now {
				*since_play = t;
				play = true;
			}
		}

		if play {
			self.pin.set_high()?;
			Timer::after(NOTE_HALF_DELAY).await;
			self.pin.set_low()?;
			Timer::after(NOTE_HALF_DELAY).await;
		}

		Ok(())
	}
}
