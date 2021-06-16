use std::sync::mpsc;

use crate::circuit::Button;
use crate::message::ButtonMessage;

pub fn poll_buttons(button_sender: mpsc::Sender<ButtonMessage>, mut buttons: Vec<Button>) -> sysfs_gpio::Result<()> {
	loop {
		for i in 0..buttons.len() {
			match buttons[i].poll(0)? {
				Some(true) => button_sender.send(ButtonMessage::Press(i)),
				Some(false) => button_sender.send(ButtonMessage::Release(i)),
				None => Ok(()),
			}.unwrap();
		}
	}
}
