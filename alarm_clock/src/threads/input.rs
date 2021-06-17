use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::{ Duration, Instant };

use rppal::gpio::{ Gpio, Level };

use crate::circuit::Button;
use crate::message::ButtonMessage;

struct Input {
	input_type: InputType,
	bounce_time: Duration,
	last_event: Option<Instant>,
}

impl Input {
	fn new(input_type: InputType) -> Self {
		let bounce_time = match input_type {
			InputType::Button(_) => Duration::from_millis(200),
		};

		Self { input_type, bounce_time, last_event: None }
	}
}

enum InputType {
	Button(u8),
}

pub fn poll_inputs(
	button_sender: mpsc::Sender<ButtonMessage>,
	buttons: Vec<Button>,
) -> rppal::gpio::Result<()> {
	let gpio = Gpio::new()?;

	let inputs: BTreeMap<_,_> = buttons.iter()
		.enumerate()
		.map(|(i, button)| (
			button.input_pin().pin(),
			Input::new(InputType::Button(i as u8))
		)).collect();

	let pins: Vec<_> = buttons.iter()
		.map(|button| button.input_pin())
		.collect();

	loop {
		if let Some((pin, level)) = gpio.poll_interrupts(&pins, false, None)? {
			let input = inputs.get(&pin.pin()).unwrap();

			if input.last_event.map_or(true, |e| e + input.bounce_time >= Instant::now()) {
				match (&input.input_type, level) {
					(InputType::Button(i), Level::High) => button_sender.send(ButtonMessage::Press(*i)),
					(InputType::Button(i), Level::Low) => button_sender.send(ButtonMessage::Release(*i)),
				}.unwrap();
			}
		}
	}
}
