use std::convert::TryInto;
use std::sync::mpsc;
use std::time::Duration;

use crate::circuit::Alphanum;
use crate::message::AlphanumMessage;

pub fn alphanum_thread(
	mut alphanum: Alphanum,
	receiver: mpsc::Receiver<AlphanumMessage>,
	scroll_delay: Duration,
) {
	let mut text;
	let mut iter = None;

	loop {
		let msg = receiver.recv_timeout(scroll_delay);

		match msg {
			Ok(AlphanumMessage::Text(t)) => {
				text = t + "    ";
				iter = Some(text.chars().cycle())
			}
			Ok(AlphanumMessage::Empty) => {
				iter = None;
			}
			Err(_) => (),
		}

		match iter {
			Some(ref mut iter) => {
				let chars = iter.clone().take(4);
				alphanum.display(&chars.collect::<Vec<_>>().try_into().unwrap()).unwrap();
				iter.next();
				std::thread::sleep(Duration::from_millis(5));
			}
			None => {
				alphanum.display(&[' ', ' ', ' ', ' ']).unwrap();
			}
		}
	}
}
