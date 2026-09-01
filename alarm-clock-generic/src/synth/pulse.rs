use core::cmp::{max, min};

use embassy_time::{Duration, Instant};

use crate::synth::{Pulse, Sound, SynthConfig, SynthUpdater};

/// Synthesizes amplitude with pulse width, and frequency with pulse frequency. This is a fast
/// and decent sounding synthesis but lacks any accuracy with regards to destructive
/// interference.
pub struct PulseUpdater;

impl SynthUpdater for PulseUpdater {
	fn update<'a>(
		&mut self,
		synth_config: &SynthConfig,
		time_range: (Instant, Instant),
		sounds: impl Iterator<Item = &'a mut Sound>,
	) -> Pulse {
		let mut on_period = 0.0;

		let mut until_next = Duration::from_ticks(u64::MAX);

		for sound in sounds {
			let instrument_config = &synth_config.instrument_config(sound.instrument);

			let sound_until =
				Duration::from_ticks(time_range.0.as_ticks() % sound.period.as_ticks());
			let mut sound_next_time = time_range.0 + sound_until;

			if sound_next_time < time_range.1 {
				sound.amplitude.evolve(
					instrument_config.decay_constant,
					instrument_config.release_decay_constant,
					sound.period,
				);

				on_period += sound.amplitude.amplitude();

				sound_next_time += sound_until;
			}

			until_next = min(
				time_range
					.1
					.checked_duration_since(sound_next_time)
					.unwrap_or(Duration::from_ticks(0)),
				until_next,
			);
		}

		let on_period = min(
			Duration::from_micros(synth_config.max_note_half_delay_us),
			Duration::from_ticks(max(0, on_period as u64)),
		);

		let off_period = max(
			Duration::from_micros(synth_config.max_note_half_delay_us),
			until_next
				.checked_sub(on_period)
				.unwrap_or(Duration::from_ticks(0)),
		);

		Pulse {
			on: on_period,
			off: off_period,
		}
	}
}
