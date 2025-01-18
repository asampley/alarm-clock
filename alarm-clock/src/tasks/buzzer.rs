use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Receiver;
use embedded_hal::digital::OutputPin;

use crate::circuit::{buzzer, hal};
use crate::error;
use crate::message::BuzzerMessage;
use crate::MIDI_NOTE_CAPACITY;

const BUZZER_NOTES: usize = 32;

#[embassy_executor::task]
pub async fn update_buzzer(
	note_receiver: Receiver<'static, CriticalSectionRawMutex, BuzzerMessage, MIDI_NOTE_CAPACITY>,
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

fn apply_message<P: OutputPin, const N: usize>(buzzer: &mut buzzer::Buzzer<P, N>, message: BuzzerMessage) {
	match message {
		BuzzerMessage::Note(note) if note.on() => {
			let _ = buzzer
				.add_note(note)
				.inspect_err(|_| error!("Buffer full when adding note"));
		}
		BuzzerMessage::Note(note) => buzzer.remove_note(&note),
		BuzzerMessage::Clear => buzzer.clear(),
	}
}
