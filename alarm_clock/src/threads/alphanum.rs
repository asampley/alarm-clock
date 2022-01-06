use std::convert::TryInto;
use std::sync::mpsc;
use std::time::Duration;

use crate::{ CONFIG, ClockTime };

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
) -> rppal::i2c::Result<()> {
	let mut text; // must keep text in scope to create an iterator
	let mut text_mode = TextMode::Time;

	loop {
		let msg = receiver.recv_timeout(Duration::from_millis(CONFIG.read().scroll_delay_ms));

		match msg {
			Ok(msg) => match msg {
				AlphanumMessage::Static(chars) => {
					alphanum.display(&chars)?;
					text_mode = TextMode::Static(chars);
				}
				AlphanumMessage::Loop(t) => {
					text = t + "    ";
					let offset = CONFIG.read().text_offset.rem_euclid(
						text.len().try_into().unwrap_or(i8::MAX)
					).try_into().unwrap();
					text_mode = TextMode::Iter(text.chars().cycle().skip(offset));
				}
				AlphanumMessage::Time => {
					text_mode = TextMode::Time;
				}
				AlphanumMessage::Empty => {
					text_mode = TextMode::Static(BLANKS);
					alphanum.display(&BLANKS)?;
				}
				AlphanumMessage::Blink(blink_rate) => {
					alphanum.blink_rate(blink_rate)?;
				}
			}
			Err(_) => (),
		}

		match text_mode {
			TextMode::Time => {
				let clock_time = ClockTime::now();

				alphanum.display(&clock_time.as_chars())?;
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
