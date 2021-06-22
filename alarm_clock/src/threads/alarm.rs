use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::{ ALARM_TIME, ALARM_SONG, ClockTime };
use crate::message::PlayerMessage;

pub fn alarm_thread(
	player_sender: Sender<PlayerMessage>,
) -> rppal::i2c::Result<()> {
	let mut before = ClockTime::now();

	loop {
		std::thread::sleep(Duration::from_millis(100));

		let after = ClockTime::now();

		if let Some(alarm_time) = *ALARM_TIME.read() {
			if before != alarm_time && after == alarm_time {
				if let Some(song) = &*ALARM_SONG.read() {
					player_sender.send(PlayerMessage::Loop(song.clone()))
						.expect("Unable to play alarm song");
				}
			}
		}

		before = after;
	}
}


