use std::sync::mpsc;
use std::sync::mpsc::TryRecvError::*;
use std::time;

use crate::circuit::Buzzer;
use crate::message::BuzzerMessage;
use crate::note::MidiNote;

pub fn update_buzzer(note_receiver: mpsc::Receiver<BuzzerMessage>, mut buzzer: Buzzer<MidiNote>) -> f64 {
	let start = time::Instant::now();
	let mut updates: u64 = 0;

	loop {
		updates += 1;

		buzzer.update();
		match note_receiver.try_recv() {
			Ok(msg) => match msg {
				BuzzerMessage::Note { on: true, note } => buzzer.add_note(note),
				BuzzerMessage::Note { on: false, note } => buzzer.remove_note(&note),
				BuzzerMessage::Clear => buzzer.clear(),
			}
			Err(err) => match err {
				Disconnected => {
					buzzer.clear();
					break;
				}
				Empty => (),
			}
		}
	}

	updates as f64 / start.elapsed().as_secs() as f64
}
