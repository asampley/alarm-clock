use embassy_time::Timer;
use embedded_hal::digital::OutputPin;

use crate::circuit::buzzer::Buzzer;
use crate::message::SynthMessage;
use crate::synth::Synth;
use crate::{MIDI_NOTE_CAPACITY, SYNTH_NOTES};
use crate::{Receiver, error};

#[embassy_executor::task]
pub async fn update_buzzer(
	note_receiver: Receiver<SynthMessage, MIDI_NOTE_CAPACITY>,
	mut buzzer: Buzzer,
	mut synth: Synth<SYNTH_NOTES>,
) {
	loop {
		if synth.is_empty() {
			apply_message(&mut synth, note_receiver.receive().await)
		} else {
			let _ = drive_buzzer(&mut synth, &mut buzzer)
				.await
				.inspect_err(|e| error!("Error while driving buzzer: {:?}", e));

			if let Ok(message) = note_receiver.try_receive() {
				apply_message(&mut synth, message);
			}
		}
	}
}

async fn drive_buzzer<const N: usize>(
	synth: &mut Synth<N>,
	buzzer: &mut Buzzer,
) -> Result<(), crate::circuit::buzzer::Error> {
	let pulse = synth.update();
	buzzer.set_high()?;
	embassy_time::block_for(pulse.on);
	buzzer.set_low()?;
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
