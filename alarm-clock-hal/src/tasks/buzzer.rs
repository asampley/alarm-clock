use embassy_executor::SpawnToken;
use embassy_time::{Instant, Timer};
use embedded_hal::digital::{ErrorType, OutputPin};

use crate::SYNTH_NOTES;
use crate::channel::MidiNoteReceiver;
use crate::channel::message::SynthMessage;
use crate::circuit::buzzer::Buzzer;
use crate::error;
use crate::synth::Synth;

pub type UpdateBuzzerTask<S, P> =
	fn(MidiNoteReceiver, Buzzer<P>, Synth<SYNTH_NOTES>) -> SpawnToken<S>;

pub async fn update_buzzer<Pin: OutputPin + ErrorType<Error: defmt::Format>>(
	note_receiver: MidiNoteReceiver,
	mut buzzer: Buzzer<Pin>,
	mut synth: Synth<SYNTH_NOTES>,
) {
	let mut last_time = Instant::from_ticks(0);

	loop {
		if !synth.will_play() {
			apply_message(&mut synth, note_receiver.receive().await);

			// time starts now when playing starts
			if synth.will_play() {
				last_time = Instant::now();
			}
		} else {
			let new_time = Instant::now();

			let _ = drive_buzzer(&mut synth, &mut buzzer, (last_time, new_time))
				.await
				.inspect_err(|e| error!("Error while driving buzzer: {:?}", e));

			if let Ok(message) = note_receiver.try_receive() {
				apply_message(&mut synth, message);
			}

			last_time = new_time;
		}
	}
}

async fn drive_buzzer<Pin: OutputPin, const N: usize>(
	synth: &mut Synth<N>,
	buzzer: &mut Buzzer<Pin>,
	time_range: (Instant, Instant),
) -> Result<(), Pin::Error> {
	let pulse = synth.update(time_range);
	if pulse.on.as_ticks() > 0 {
		buzzer.set_high()?;
		embassy_time::block_for(pulse.on);
		buzzer.set_low()?;
	}
	Timer::after(pulse.off).await;

	Ok(())
}

fn apply_message<const N: usize>(synth: &mut Synth<N>, message: SynthMessage) {
	match message {
		SynthMessage::Midi { channel, message } => {
			let _ = synth
				.process_midi(channel, message)
				.inspect_err(|e| error!("Error while processing midi message: {:?}", e));
		}
		SynthMessage::Clear => synth.stop(),
	}
}
