use core::cmp::{max, min};

use embassy_time::{Duration, Instant};

use crate::synth::{Pulse, Sound, SynthConfig, SynthUpdater, cosr};

/// Synthesizes amplitude with pulse width, and frequency with pulse frequency. This is a fast
/// and decent sounding synthesis but lacks any accuracy with regards to destructive
/// interference.
pub struct SampleUpdater;

impl SynthUpdater for SampleUpdater {
	fn update<'a>(
		&mut self,
		synth_config: &SynthConfig,
		time_range: (Instant, Instant),
		sounds: impl Iterator<Item = &'a mut Sound>,
	) -> Pulse {
		let mut on_period = 0.0;

		for sound in sounds {
			let instrument_config = synth_config.instrument_config(sound.instrument);

			let area = cosr(time_range.1.as_ticks() as f64 / sound.period.as_ticks() as f64)
				- cosr(time_range.0.as_ticks() as f64 / sound.period.as_ticks() as f64);

			sound.amplitude.evolve(
				instrument_config.decay_constant,
				instrument_config.release_decay_constant,
				sound.period,
			);

			on_period += area * sound.amplitude.amplitude();
		}

		let on_period = min(
			Duration::from_micros(synth_config.max_note_half_delay_us),
			Duration::from_ticks(max(0, libm::round(on_period) as u64)),
		);

		Pulse {
			on: on_period,
			off: Duration::from_micros(synth_config.max_note_half_delay_us),
		}
	}
}
