#[cfg(target_arch = "xtensa")]
mod xtensa {
	use crate::circuit::{button::Button, buzzer::Buzzer, dht::Dht11};
	use embassy_time::Duration;

	use esp_hal::gpio::{AnyPin, DriveMode, Input, InputConfig, Level, Output, OutputConfig, Pull};

	impl From<AnyPin<'static>> for Dht11 {
		fn from(pin: AnyPin<'static>) -> Self {
			Self::new(
				Output::new(
					pin,
					Level::High,
					OutputConfig::default()
						.with_drive_mode(DriveMode::OpenDrain)
						.with_pull(Pull::Up),
				)
				.into_flex(),
			)
		}
	}

	impl From<AnyPin<'static>> for Buzzer {
		fn from(pin: AnyPin<'static>) -> Self {
			Self::new(Output::new(pin, Level::Low, OutputConfig::default()))
		}
	}

	impl From<(AnyPin<'static>, Duration)> for Button {
		fn from((pin, bounce_time): (AnyPin<'static>, Duration)) -> Self {
			Self::new(
				Input::new(pin, InputConfig::default().with_pull(Pull::Up)),
				bounce_time,
			)
		}
	}
}
