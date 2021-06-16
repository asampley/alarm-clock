use super::gpio::{Gpio, GpioError};

use std::time;

use sysfs_gpio::{Pin, PinPoller, Edge};

pub struct Button {
//	pin: Pin,
	poller: PinPoller,
	wait: time::Duration,
	last_poll: time::Instant,
	last_value: bool,
}

impl Button {
	pub fn new(pin_num: u64, poll_wait: time::Duration) -> Result<Self, GpioError> {
		let pin = Pin::new(pin_num);

		// export pin with edges for polling
		pin.export_edge(Edge::BothEdges)?;

		// set pin to pull up resistor for the button
		pin.pull_up()?;

		Ok(Button {
//			pin: pin,
			poller: pin.get_poller()?,
			wait: poll_wait,
			last_poll: time::Instant::now() - poll_wait,
			last_value: pin.get_value()? > 0,
		})
	}

//	pub fn read(&self) -> sysfs_gpio::Result<bool> {
//		Ok(self.pin.get_value()? > 0)
//	}

	pub fn poll(&mut self, timeout_ms: isize) -> sysfs_gpio::Result<Option<bool>> {
		if self.last_poll + self.wait < time::Instant::now() {
			self.last_value = loop {
				match self.poller.poll(timeout_ms).map(|opt| opt.map(|v| v > 0))? {
					None => return Ok(None),
					Some(v) => if v != self.last_value { break v },
				}
			}
		}

		Ok(Some(self.last_value))
	}
}
