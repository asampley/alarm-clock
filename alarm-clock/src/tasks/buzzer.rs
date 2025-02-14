use embassy_time::Timer;
use embedded_hal::digital::OutputPin;

use crate::circuit::{buzzer, hal};
use crate::message::SynthMessage;
use crate::synth::Synth;
use crate::{error, Receiver};
use crate::{MIDI_NOTE_CAPACITY, SYNTH_NOTES};

#[embassy_executor::task]
pub async fn update_buzzer(
	note_receiver: Receiver<SynthMessage, MIDI_NOTE_CAPACITY>,
	mut buzzer: hal::Buzzer,
	mut synth: Synth<SYNTH_NOTES>,
) {
	loop {
		if synth.is_empty() {
			apply_message(&mut synth, note_receiver.receive().await)
		} else {
			let _ = drive_buzzer(&mut synth, &mut buzzer)
				.await
				.inspect_err(|e| error!("Error while driving buzzer: {:?}", e));

			match note_receiver.try_receive() {
				Ok(message) => apply_message(&mut synth, message),
				Err(_) => (),
			}
		}
	}
}

async fn drive_buzzer<P: OutputPin, const N: usize>(
	synth: &mut Synth<N>,
	buzzer: &mut buzzer::Buzzer<P>,
) -> Result<(), P::Error> {
	if let Some(pulse) = synth.update() {
		buzzer.set_high()?;
		embassy_time::block_for(pulse.on);
		buzzer.set_low()?;
		Timer::after(pulse.off).await;
	}

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
