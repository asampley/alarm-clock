#[cfg(target_arch = "xtensa")]
mod xtensa {
	use crate::circuit::{
		alphanum::Alphanum,
		button::Button,
		buzzer::Buzzer,
		dht::Dht11,
	};
	use embassy_time::Duration;

	use esp_hal::gpio::{AnyPin, DriveMode, Input, InputConfig, Level, Output, OutputConfig, Pull};
	use esp_hal::i2c::master::AnyI2c;

	impl From<AnyPin<'static>> for Dht11 {
		#[define_opaque(crate::circuit::dht::Pin)]
		fn from(pin: AnyPin<'static>) -> Self {
			Self::new(Output::new(
				pin,
				Level::High,
				OutputConfig::default()
					.with_drive_mode(DriveMode::OpenDrain)
					.with_pull(Pull::Up)
			).into_flex())
		}
	}

	impl From<AnyPin<'static>> for Buzzer {
		#[define_opaque(crate::circuit::buzzer::Pin)]
		fn from(pin: AnyPin<'static>) -> Self {
			Self::new(Output::new(pin, Level::Low, OutputConfig::default()))
		}
	}

	impl From<(AnyPin<'static>, Duration)> for Button {
		#[define_opaque(crate::circuit::button::Pin)]
		fn from((pin, bounce_time): (AnyPin<'static>, Duration)) -> Self {
			Self::new(Input::new(pin, InputConfig::default().with_pull(Pull::Up)), bounce_time)
		}
	}

	impl Alphanum {
		#[define_opaque(crate::circuit::alphanum::I2c)]
		pub fn new_esp_hal(
			pin: AnyI2c<'static>,
			sda: AnyPin<'static>,
			scl: AnyPin<'static>,
		) -> Result<Self, crate::circuit::alphanum::Error>
		{
			Self::new(
				esp_hal::i2c::master::I2c::new(pin, Default::default())
					.unwrap()
					.with_sda(sda)
					.with_scl(scl)
					.into_async(),
			)
		}
	}
}
