use std::convert::TryInto;
use std::sync::mpsc;
use std::time::{ Instant, Duration };

use crate::TIME_ZERO;

use crate::circuit::Alphanum;
use crate::message::AlphanumMessage;

const BLANKS: [char; 4] = [' ', ' ', ' ', ' '];

enum TextMode<I: Iterator<Item = char>> {
	Time,
	Static([char; 4]),
	Iter(I),
}

pub fn alphanum_thread(
	mut alphanum: Alphanum,
	receiver: mpsc::Receiver<AlphanumMessage>,
	scroll_delay: Duration,
) -> rppal::i2c::Result<()> {
	let mut text; // must keep text in scope to create an iterator
	let mut text_mode = TextMode::Time;

	loop {
		let msg = receiver.recv_timeout(scroll_delay);

		match msg {
			Ok(msg) => match msg {
				AlphanumMessage::Static(chars) => {
					text_mode = TextMode::Static(chars);
				}
				AlphanumMessage::Text(t) => {
					text = t + "    ";
					text_mode = TextMode::Iter(text.chars().cycle());
				}
				AlphanumMessage::Time => {
					text_mode = TextMode::Time;
				}
				AlphanumMessage::Empty => {
					text_mode = TextMode::Static(BLANKS);
					alphanum.display(&BLANKS)?;
				}
			}
			Err(_) => (),
		}

		match text_mode {
			TextMode::Time => {
				let seconds = (Instant::now() - *TIME_ZERO.read().unwrap()).as_secs();
				let minutes = (seconds / 60) % 60;
				let hours = (seconds / 60 / 60) % 24;

				alphanum.display(&format!("{:02}{:02}", hours, minutes).chars().collect::<Vec<_>>().try_into().unwrap())?;
			}
			TextMode::Static(_) => (),
			TextMode::Iter(ref mut iter) => {
				let chars = iter.clone().take(4).collect::<Vec<_>>().try_into().unwrap();
				alphanum.display(&chars)?;
				iter.next();
			}
		}
	}
}
