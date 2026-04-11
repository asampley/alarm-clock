#[cfg(feature = "synth-pulse")]
pub mod pulse;

#[cfg(feature = "synth-sample")]
pub mod sample;

use defmt::Format;
use embassy_time::{Duration, Instant};
use enum_dispatch::enum_dispatch;
use heapless::index_map::FnvIndexMap;
use midly::{
	MidiMessage,
	num::{u4, u7},
};

/// Chosen because humans should be able to hear at most a 19kHz, or 1/52us
///
/// Too small a delay reduces volume drastically
const TICK_S: f64 = 1.0 / embassy_time::TICK_HZ as f64;

#[pre_table::sin_table]
static SIN: [f64; 256];
const SIN_L: usize = SIN.len();
const SIN_LF: f64 = SIN_L as f64;

const fn sinr(circle_ratio: f64) -> f64 {
	SIN[(circle_ratio * SIN_LF) as usize % SIN_L]
}

const fn cosr(circle_ratio: f64) -> f64 {
	sinr(circle_ratio + 0.5)
}

fn frequency(key: u7) -> f64 {
	#[pre_table::freq_table]
	static F: [f64; 128];

	F[usize::from(key.as_int())]
}

#[enum_dispatch(SynthUpdater)]
pub enum DynSynthUpdater {
	#[cfg(feature = "synth-pulse")]
	Pulse(pulse::PulseUpdater),
	#[cfg(feature = "synth-sample")]
	Sample(sample::SampleUpdater),
}

#[derive(Clone, Copy, Format)]
pub enum Updater {
	#[cfg(feature = "synth-pulse")]
	Pulse,
	#[cfg(feature = "synth-sample")]
	Sample,
}

pub struct Synth<const NOTES: usize> {
	instruments: [u7; 16],
	notes: FnvIndexMap<SoundKey, Sound, NOTES>,

	config: SynthConfig,
}

#[enum_dispatch]
pub trait SynthUpdater {
	fn update<'a>(
		&mut self,
		synth_config: &SynthConfig,
		time_range: (Instant, Instant),
		sounds: impl Iterator<Item = &'a mut Sound>,
	) -> Pulse;
}

#[derive(Clone, Format)]
pub struct SynthConfig {
	pub max_note_half_delay_us: u64,
	pub updater: Updater,
	pub pluck: InstrumentConfig,
	pub hold: InstrumentConfig,
}

#[derive(Clone, Format)]
pub struct InstrumentConfig {
	pub sustain_ratio: f64,
	pub decay_constant: f64,
	pub release_decay_constant: f64,
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
pub struct Sound {
	#[defmt(Debug2Format)]
	instrument: u7,
	period: Duration,
	amplitude: Amplitude,
}

#[derive(Format)]
enum Amplitude {
	Decay {
		amplitude: f64,
		sustain_transition: f64,
	},
	Sustain {
		amplitude: f64,
	},
	Release {
		amplitude: f64,
	},
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

	fn add_note(&mut self, sound_key: SoundKey, vel: u7) -> Result<(), ()> {
		let on_period_ticks = self.config.max_note_half_delay_us as f64 * vel.as_int() as f64
			/ u7::max_value().as_int() as f64;

		let instrument = self.instruments[usize::from(sound_key.channel.as_int())];

		let sound = Sound {
			instrument,
			period: Duration::from_micros((1_000_000.0 / frequency(sound_key.key)) as u64),
			amplitude: Amplitude::Decay {
				amplitude: on_period_ticks,
				sustain_transition: on_period_ticks
					* self.config.instrument_config(instrument).sustain_ratio,
			},
		};

		self.notes
			.insert(sound_key, sound)
			.map(|_| ())
			.map_err(|_| ())
	}

	fn release_note(&mut self, sound_key: &SoundKey) {
		if let Some(s) = self.notes.get_mut(sound_key) {
			s.amplitude.release();
		}
	}

	pub fn will_play(&self) -> bool {
		!self.notes.is_empty()
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

	pub fn process_controller(&mut self, _channel: u4, controller: u7, _value: u7) {
		const CONTROLLER_ALL_NOTES_OFF: u7 = u7::new(127);

		match controller {
			CONTROLLER_ALL_NOTES_OFF => self.stop(),
			_ => (),
		}
	}

	pub fn stop(&mut self) {
		self.instruments = [0.into(); 16];
		self.notes.clear()
	}

	pub fn update(&mut self, time_range: (Instant, Instant)) -> Pulse {
		let pulse =
			self.config
				.updater
				.updater()
				.update(&self.config, time_range, self.notes.values_mut());

		self.notes.retain(|_, sound| !sound.amplitude.done());

		pulse
	}
}

impl Updater {
	fn updater(&self) -> DynSynthUpdater {
		match self {
			#[cfg(feature = "synth-pulse")]
			Self::Pulse => DynSynthUpdater::Pulse(pulse::PulseUpdater),
			#[cfg(feature = "synth-pulse")]
			Self::Sample => DynSynthUpdater::Sample(sample::SampleUpdater),
		}
	}
}

impl Amplitude {
	const fn amplitude(&self) -> f64 {
		match self {
			Self::Decay { amplitude, .. } => *amplitude,
			Self::Sustain { amplitude } => *amplitude,
			Self::Release { amplitude } => *amplitude,
		}
	}

	fn release(&mut self) {
		match self {
			Self::Decay { amplitude, .. } => {
				*self = Self::Release {
					amplitude: *amplitude,
				}
			}
			Self::Sustain { amplitude } => {
				*self = Self::Release {
					amplitude: *amplitude,
				}
			}
			Self::Release { .. } => (),
		};
	}

	const fn done(&self) -> bool {
		self.amplitude() <= 0.0
	}

	fn evolve(&mut self, decay_constant: f64, release_decay_constant: f64, elapsed: Duration) {
		match self {
			Amplitude::Decay {
				amplitude,
				sustain_transition,
			} => {
				*amplitude *= libm::exp2(-(elapsed.as_ticks() as f64) * decay_constant);

				if amplitude < sustain_transition {
					*self = Amplitude::Sustain {
						amplitude: *sustain_transition,
					}
				}
			}
			Amplitude::Sustain { .. } => (),
			Amplitude::Release { amplitude } => {
				// exponential shifted down to intersect the x-axis
				*amplitude += 1.0;
				*amplitude *= libm::exp2(-(elapsed.as_ticks() as f64) * release_decay_constant);
				*amplitude -= 1.0;
			}
		}
	}
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
