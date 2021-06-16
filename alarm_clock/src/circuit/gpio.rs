use std::ffi::OsStr;
use std::fmt;
use std::process::Command;

use sysfs_gpio::{Pin, Direction, Edge};

pub trait Gpio {
	type Error;

	fn export_direction(&self, direction: Direction) -> Result<(), Self::Error>;
	fn export_edge(&self, edge: Edge) -> Result<(), Self::Error>;
	fn pull_up(&self) -> Result<(), Self::Error>;
	fn pull_down(&self) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub enum GpioError {
	StdIoError(std::io::Error),
	SysfsGpioError(sysfs_gpio::Error),
	GpioCommandError(std::process::Output),
}

impl fmt::Display for GpioError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

impl std::error::Error for GpioError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		use GpioError::*;

		match self {
			StdIoError(ref e) => Some(e),
			SysfsGpioError(ref e) => Some(e),
			GpioCommandError(_) => None,
		}
	}
}

impl From<sysfs_gpio::Error> for GpioError {
	fn from(err: sysfs_gpio::Error) -> Self {
		GpioError::SysfsGpioError(err)
	}
}

impl From<std::io::Error> for GpioError {
	fn from(err: std::io::Error) -> Self {
		GpioError::StdIoError(err)
	}
}

impl Gpio for Pin {
	type Error = GpioError;

	fn export_direction(&self, direction: Direction) -> Result<(), Self::Error> {
		gpio(&[
			"export",
			&format!("{}", self.get_pin_num()),
			match direction {
				Direction::In => "in",
				Direction::Out => "out",
				Direction::High => "high",
				Direction::Low => "low",
			},
		])
	}

	fn export_edge(&self, edge: Edge) -> Result<(), Self::Error> {
		gpio(&[
			"edge",
			&format!("{}", self.get_pin_num()),
			match edge {
				Edge::NoInterrupt => "none",
				Edge::RisingEdge => "rising",
				Edge::FallingEdge => "falling",
				Edge::BothEdges => "both",
			},
		])
	}

	fn pull_up(&self) -> Result<(), Self::Error> {
		gpio(&["-g", "mode", &format!("{}", self.get_pin_num()), "up"])
	}

	fn pull_down(&self) -> Result<(), Self::Error> {
		gpio(&["-g", "mode", &format!("{}", self.get_pin_num()), "down"])
	}
}

fn gpio<I, S>(args: I) -> Result<(), GpioError> 
where
	I: IntoIterator<Item = S>,
	S: AsRef<OsStr>,
{
	let output = Command::new("/usr/bin/gpio")
		.args(args)
		.output()?;

	if !output.status.success() {
		Err(GpioError::GpioCommandError(output))
	} else {
		Ok(())
	}
}
