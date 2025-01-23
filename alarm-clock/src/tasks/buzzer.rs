use embedded_hal::digital::OutputPin;

use crate::circuit::{buzzer, hal};
use crate::message::SynthMessage;
use crate::MIDI_NOTE_CAPACITY;
use crate::{error, Receiver};

const BUZZER_NOTES: usize = 32;

#[embassy_executor::task]
pub async fn update_buzzer(
	note_receiver: Receiver<SynthMessage, MIDI_NOTE_CAPACITY>,
	mut buzzer: hal::Buzzer<BUZZER_NOTES>,
) {
	loop {
		if buzzer.is_empty() {
			apply_message(&mut buzzer, note_receiver.receive().await)
		} else {
			let _ = buzzer.update().await.inspect_err(|e| error!("{:?}", e));

			match note_receiver.try_receive() {
				Ok(message) => apply_message(&mut buzzer, message),
				Err(_) => (),
			}
		}
	}
}

fn apply_message<P: OutputPin, const N: usize>(
	buzzer: &mut buzzer::Buzzer<P, N>,
	message: SynthMessage,
) {
	match message {
		SynthMessage::Midi { channel, message } => {
			let _ = buzzer
				.synth
				.process_midi(channel, message)
				.inspect_err(|e| error!("Error while processing midi message: {:?}", e));
		}
		SynthMessage::Clear => buzzer.clear(),
	}
}
