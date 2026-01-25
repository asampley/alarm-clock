use core::cmp::{max, min};

use defmt::Format;
use embassy_time::{Duration, Instant};
use heapless::index_map::FnvIndexMap;
use midly::{
	MidiMessage,
	num::{u4, u7},
};
use serde::Deserialize;

/// Chosen because humans should be able to hear at most a 19kHz, or 1/52us
///
/// Too small a delay reduces volume drastically
const TICK_S: f64 = 1.0 / embassy_time::TICK_HZ as f64;

#[pre_table::sin_table]
static SIN: [f64; 256];
const SIN_L: usize = SIN.len();
const SIN_LF: f64 = SIN_L as f64;

const fn sin(circle_ratio: f64) -> f64 {
	SIN[(circle_ratio * SIN_LF) as usize % SIN_L]
}

const fn cos(circle_ratio: f64) -> f64 {
	SIN[((circle_ratio + 0.5) * SIN_LF) as usize % SIN_L]
}

fn frequency(key: u7) -> f64 {
	#[pre_table::freq_table]
	static F: [f64; 128];

	F[usize::from(key.as_int())]
}

#[derive(Clone, Deserialize, Format)]
pub struct SynthConfig {
	max_note_half_delay_us: u64,
	pluck: InstrumentConfig,
	hold: InstrumentConfig,
}

#[derive(Clone, Deserialize, Format)]
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
			0..=16 | 25..=40 | 105..=109 | 113..=121 | 123..=125 | 128 => &self.pluck,
			17..=24 | 41..=104 | 110..=112 | 122 | 126..=127 => &self.hold,
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

		match message {
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
			}
			_ => (),
		}

		Ok(())
	}

	fn process_controller(&mut self, _channel: u4, controller: u7, _value: u7) {
		const CONTROLLER_ALL_NOTES_OFF: u7 = u7::new(127);

		match controller {
			CONTROLLER_ALL_NOTES_OFF => self.stop(),
			_ => (),
		}
	}

	fn add_note(&mut self, sound_key: SoundKey, vel: u7) -> Result<(), ()> {
		let on_period_ticks = self.config.max_note_half_delay_us as f64 * vel.as_int() as f64
			/ u7::max_value().as_int() as f64;

		let instrument = self.instruments[usize::from(sound_key.channel.as_int())];

		let sound = Sound {
			instrument,
			period: Duration::from_micros((1_000_000.0 / frequency(sound_key.key)) as u64),
			amplitude: Amplitude::Decay {
				on_period_ticks,
				sustain_transition: on_period_ticks
					* self.config.instrument_config(instrument).sustain_ratio,
			},
		};

		self.notes
			.insert(sound_key, (sound, Instant::now()))
			.map(|_| ())
			.map_err(|_| ())
	}

	fn release_note(&mut self, sound_key: &SoundKey) {
		if let Some((s, _)) = self.notes.get_mut(sound_key) {
			s.amplitude.release();
		}
	}

	pub fn stop(&mut self) {
		self.instruments = [0.into(); 16];
		self.notes.clear()
	}

	/// Returns how wide the wave form should be which is approximately amplitude
	///
	/// Even if empty, this will return an off pulse.
	pub fn update(&mut self) -> Pulse {
		let now = Instant::now();

		let mut on_period = 0.0;

		for (_, (sound, since_play)) in &mut self.notes {
			let instrument_config = self.config.instrument_config(sound.instrument);

			let t = *since_play + sound.period;

			if t < now {
				sound.amplitude.evolve(
					instrument_config.decay_constant,
					instrument_config.release_decay_constant,
					sound.period,
				);

				*since_play = t;

				on_period += sound.amplitude.on_period_ticks();
			}
		}

		let on_period = min(
			Duration::from_micros(self.config.max_note_half_delay_us),
			Duration::from_ticks(max(0, on_period as u64)),
		);

		self.notes.retain(|_, (sound, _)| !sound.amplitude.done());

		Pulse {
			on: on_period,
			off: Duration::from_micros(self.config.max_note_half_delay_us),
		}
	}
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
struct SoundKey {
	channel: u4,
	key: u7,
}

#[derive(Format)]
pub struct Pulse {
	pub on: Duration,
	pub off: Duration,
}

#[derive(Format)]
struct Sound {
	#[defmt(Debug2Format)]
	instrument: u7,
	period: Duration,
	amplitude: Amplitude,
}

#[derive(Format)]
enum Amplitude {
	Decay {
		on_period_ticks: f64,
		sustain_transition: f64,
	},
	Sustain {
		on_period_ticks: f64,
	},
	Release {
		on_period_ticks: f64,
	},
}

impl Amplitude {
	const fn on_period_ticks(&self) -> f64 {
		match self {
			Self::Decay {
				on_period_ticks, ..
			} => *on_period_ticks,
			Self::Sustain { on_period_ticks } => *on_period_ticks,
			Self::Release { on_period_ticks } => *on_period_ticks,
		}
	}

	fn release(&mut self) {
		match self {
			Self::Decay {
				on_period_ticks, ..
			} => {
				*self = Self::Release {
					on_period_ticks: *on_period_ticks,
				}
			}
			Self::Sustain { on_period_ticks } => {
				*self = Self::Release {
					on_period_ticks: *on_period_ticks,
				}
			}
			Self::Release { .. } => (),
		};
	}

	const fn done(&self) -> bool {
		self.on_period_ticks() <= 0.0
	}

	fn evolve(&mut self, decay_constant: f64, release_decay_constant: f64, elapsed: Duration) {
		match self {
			Amplitude::Decay {
				on_period_ticks,
				sustain_transition,
			} => {
				*on_period_ticks *= libm::exp2(-(elapsed.as_ticks() as f64) * decay_constant);

				if on_period_ticks < sustain_transition {
					*self = Amplitude::Sustain {
						on_period_ticks: *sustain_transition,
					}
				}
			}
			Amplitude::Sustain { .. } => (),
			Amplitude::Release { on_period_ticks } => {
				// exponential shifted down to intersect the x-axis
				*on_period_ticks += 1.0;
				*on_period_ticks *=
					libm::exp2(-(elapsed.as_ticks() as f64) * release_decay_constant);
				*on_period_ticks -= 1.0;
			}
		}
	}
}
