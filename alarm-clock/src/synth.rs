use core::cmp::min;

use embassy_time::{Instant, Duration};
use heapless::FnvIndexMap;
use midly::{num::{u4, u7}, MidiMessage};
use serde::Deserialize;

/// Chosen because humans should be able to hear at most a 19kHz, or 1/52us
///
/// Too small a delay reduces volume drastically
const MAX_NOTE_HALF_DELAY_US: u64 = 30;
const TICK_S: f64 = 1.0 / embassy_time::TICK_HZ as f64;

fn frequency(key: u7) -> f64 {
	#[pre_table::freq_table]
	static F: [f64; 128];

	F[usize::from(key.as_int())]
}

#[derive(Clone, Debug, Deserialize)]
pub struct SynthConfig {
	pluck: InstrumentConfig,
	hold: InstrumentConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct InstrumentConfig {
	sustain_ratio: f64,
	decay_constant: f64,
	release_decay_constant: f64,
}

pub struct Synth<const NOTES: usize> {
	instruments: [u7; 16],
	notes: FnvIndexMap<SoundKey, (Sound, Instant), NOTES>,

	config: SynthConfig,
}

impl SynthConfig {
	fn instrument_config(&self, instrument: u7) -> &InstrumentConfig {
		match instrument.as_int() {
			0..=16 | 25..=40 | 105..=109 | 113..=121 | 123..=125 | 128 => {
				&self.pluck
			}
			17..=24 | 41..=104 | 110..=112 | 122 | 126..=127 => {
				&self.hold
			}
			128.. => unreachable!(),
		}
	}
}

impl<const NOTES: usize> Synth<NOTES> {
	pub fn new(mut config: SynthConfig) -> Self {
		config.pluck.decay_constant *= TICK_S;
		config.pluck.release_decay_constant *= TICK_S;
		config.hold.decay_constant *= TICK_S;
		config.hold.release_decay_constant *= TICK_S;

		Self {
			instruments: [0.into(); 16],
			notes: Default::default(),
			config,
		}
	}

	pub fn is_empty(&self) -> bool {
		self.notes.is_empty()
	}

	pub fn process_midi(&mut self, channel: u4, message: MidiMessage) -> Result<(), ()> {
		const ZERO: u7 = u7::new(0);

		Ok(match message {
			MidiMessage::NoteOff { key, .. } | MidiMessage::NoteOn { key, vel: ZERO } => {
				self.release_note(&SoundKey { channel, key })
			}
			MidiMessage::NoteOn { key, vel } | MidiMessage::Aftertouch { key, vel } => {
				self.add_note(SoundKey { channel, key }, vel)?
			}
			MidiMessage::Controller { controller, value } => {
				self.process_controller(channel, controller, value)
			}
			MidiMessage::ProgramChange { program } => {
				self.instruments[usize::from(channel.as_int())] = program
			},
			_ => (),
		})
	}

	fn process_controller(&mut self, _channel: u4, controller: u7, _value: u7) {
		const CONTROLLER_ALL_NOTES_OFF: u7 = u7::new(127);

		match controller {
			CONTROLLER_ALL_NOTES_OFF => self.stop(),
			_ => (),
		}
	}

	fn add_note(&mut self, sound_key: SoundKey, vel: u7) -> Result<(), ()> {
		let on_period_ticks = MAX_NOTE_HALF_DELAY_US as f64
				* vel.as_int() as f64
				/ u7::max_value().as_int() as f64;

		let instrument = self.instruments[usize::from(sound_key.channel.as_int())];

		let sound = Sound {
			instrument,
			period: Duration::from_micros((1_000_000.0 / frequency(sound_key.key)) as u64),
			amplitude: Amplitude::Decay {
				on_period_ticks,
				sustain_transition: on_period_ticks * self.config.instrument_config(instrument).sustain_ratio,
			}
		};

		self.notes.insert(sound_key, (sound, Instant::now())).map(|_| ()).map_err(|_| ())
	}

	fn release_note(&mut self, sound_key: &SoundKey) {
		self.notes.get_mut(sound_key).map(|(s, _)| s.amplitude.release());
	}

	pub fn stop(&mut self) {
		self.instruments = [0.into(); 16];
		self.notes.clear()
	}

	/// Returns how wide the wave form should be which is approximately amplitude
	pub fn update(&mut self) -> Option<Pulse> {
		let now = Instant::now();

		let mut on_period = None;

		for (_, (ref mut sound, since_play)) in &mut self.notes {
			let instrument_config = self.config.instrument_config(sound.instrument);

			let t = *since_play + sound.period;

			if t < now {
				sound.amplitude.evolve(
					instrument_config.decay_constant,
					instrument_config.release_decay_constant,
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

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
struct SoundKey {
	channel: u4,
	key: u7,
}

#[derive(Debug)]
pub struct Pulse {
	pub on: Duration,
	pub off: Duration,
}

#[derive(Debug)]
struct Sound {
	instrument: u7,
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
