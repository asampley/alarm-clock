use crate::{Devices, Error};
use crate::tweaks::Config;
use crate::circuit::{
	button::Button,
	buzzer::Buzzer,
	dht::Dht11,
};
use crate::message::{ButtonDirection, ButtonFunction};

use embassy_time::Duration;

#[cfg(target_arch = "xtensa")]
pub use xtensa::*;

#[cfg(target_arch = "xtensa")]
mod xtensa {
	#[cfg(not(any(
		feature = "esp32s2",
		feature = "esp32s3",
	)))]
	compile_error!("Must enable one of the esp features on xtensa hardware");

	// import alone enables backtrace
	use esp_backtrace as _;

	use super::*;

	#[define_opaque(crate::tasks::i2c::I2c)]
	pub fn setup_hardware(config: &Config) -> Result<Devices, Error> {
		use esp_hal::gpio::Pin;

		let p = esp_hal::init(esp_hal::Config::default()
			.with_cpu_clock(esp_hal::clock::CpuClock::max())
		);

		esp_rtos::start(esp_hal::timer::timg::TimerGroup::new(p.TIMG0).timer0);

		// create buzzer controller
		let buzzer = Buzzer::from(p.GPIO14.degrade());

		let button_bounce_time = Duration::from_millis(config.button_bounce_ms);

		// create button pollers
		let buttons = [
			(ButtonFunction::Select, p.GPIO1.degrade()),
			(
				ButtonFunction::Direction(ButtonDirection::Prev),
				p.GPIO3.degrade(),
			),
			(
				ButtonFunction::Direction(ButtonDirection::Next),
				p.GPIO2.degrade(),
			),
		]
		.map(|(f, p)| (f, Button::from((p, button_bounce_time))));

		let i2c = esp_hal::i2c::master::I2c::new(p.I2C0, Default::default())
			.unwrap()
			.with_sda(p.GPIO11)
			.with_scl(p.GPIO12)
			.into_async();

		// create temp/humid sensor driver
		let humid_temp = Dht11::from(p.GPIO13.degrade());

		Ok(Devices { buzzer, buttons, humid_temp, i2c })
	}
}
