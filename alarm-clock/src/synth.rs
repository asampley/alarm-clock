use core::cmp::min;

use embassy_time::{Instant, Duration};
use heapless::FnvIndexMap;
use midly::num::u7;

use crate::note::MidiNote;

/// Chosen because humans should be able to hear at most a 19kHz, or 1/52us
///
/// Too small a delay reduces volume drastically
const MAX_NOTE_HALF_DELAY_US: u64 = 30;
const TICK_S: f64 = 1.0 / embassy_time::TICK_HZ as f64;

pub struct Synth<const NOTES: usize> {
	sustain_ratio: f64,
	decay_constant: f64,
	release_decay_constant: f64,
	notes: FnvIndexMap<u7, (Sound, Instant), NOTES>,
}

impl<const NOTES: usize> Synth<NOTES> {
	pub fn new(sustain_ratio: f64, decay_constant: f64, release_decay_constant: f64) -> Self {
		Self {
			sustain_ratio,
			decay_constant: decay_constant * TICK_S,
			release_decay_constant: release_decay_constant * TICK_S,
			notes: Default::default(),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.notes.is_empty()
	}

	pub fn add_note(&mut self, note: MidiNote) -> Result<(), ()> {
		let on_period_ticks = MAX_NOTE_HALF_DELAY_US as f64
				* note.vel.as_int() as f64
				/ u7::max_value().as_int() as f64;

		let sound = Sound {
			period: Duration::from_micros((1_000_000.0 / note.frequency()) as u64),
			amplitude: Amplitude::Decay {
				on_period_ticks,
				sustain_transition: on_period_ticks * self.sustain_ratio,
			}
		};

		self.notes.insert(note.key, (sound, Instant::now())).map(|_| ()).map_err(|_| ())
	}

	pub fn release_note(&mut self, note: &MidiNote) {
		self.notes.get_mut(&note.key).map(|(s, _)| s.amplitude.release());
	}

	pub fn stop(&mut self) {
		self.notes.clear()
	}

	/// Returns how wide the wave form should be which is approximately amplitude
	pub fn update(&mut self) -> Option<Pulse> {
		let now = Instant::now();

		let mut on_period = None;

		for (_, (ref mut sound, since_play)) in &mut self.notes {
			let t = *since_play + sound.period;

			if t < now {
				sound.amplitude.evolve(
					self.decay_constant,
					self.release_decay_constant,
					sound.period
				);

				*since_play = t;

				on_period = Some(min(
					Duration::from_micros(MAX_NOTE_HALF_DELAY_US),
					on_period.unwrap_or(Duration::from_ticks(0)) + sound.amplitude.on_period()
				));
			}
		}

		self.notes.retain(|_, (sound, _)| !sound.amplitude.done());

		on_period.map(|on| Pulse { on, off: Duration::from_micros(MAX_NOTE_HALF_DELAY_US) })
	}
}

#[derive(Debug)]
pub struct Pulse {
	pub on: Duration,
	pub off: Duration,
}

#[derive(Debug)]
struct Sound {
	period: Duration,
	amplitude: Amplitude,
}

#[derive(Debug)]
enum Amplitude {
	Decay {
		on_period_ticks: f64,
		sustain_transition: f64,
	},
	Sustain {
		on_period: Duration,
	},
	Release {
		on_period_ticks: f64,
	}
}

impl Amplitude {
	const fn on_period(&self) -> Duration {
		match self {
			Self::Decay { on_period_ticks, .. } => Duration::from_ticks(*on_period_ticks as u64),
			Self::Sustain { on_period } => *on_period,
			Self::Release { on_period_ticks } => Duration::from_ticks(*on_period_ticks as u64),
		}
	}

	fn release(&mut self) {
		match self {
			Self::Decay { on_period_ticks, .. } => *self = Self::Release {
				on_period_ticks: *on_period_ticks,
			},
			Self::Sustain { on_period } => *self = Self::Release {
				on_period_ticks: on_period.as_ticks() as f64
			},
			Self::Release { .. } => (),
		};
	}

	fn done(&self) -> bool {
		if let Self::Release { on_period_ticks } = self {
			*on_period_ticks <= 0.0
		} else {
			false
		}
	}

	fn evolve(
		&mut self,
		decay_constant: f64,
		release_decay_constant: f64,
		elapsed: Duration,
	) {
		match self {
			Amplitude::Decay { on_period_ticks, sustain_transition } => {
				*on_period_ticks *= libm::exp2(-(elapsed.as_ticks() as f64) * decay_constant);

				if on_period_ticks < sustain_transition {
					*self = Amplitude::Sustain {
						on_period: Duration::from_ticks(*sustain_transition as u64)
					}
				}
			}
			Amplitude::Sustain { .. } => (),
			Amplitude::Release { on_period_ticks } => {
				// exponential shifted down to intersect the x-axis
				*on_period_ticks += 1.0;
				*on_period_ticks *= libm::exp2(-(elapsed.as_ticks() as f64) * release_decay_constant);
				*on_period_ticks -= 1.0;
			}
		}
	}
}
