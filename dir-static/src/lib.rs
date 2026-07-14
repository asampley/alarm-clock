#![no_std]

pub use dir_static_macro::*;

#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct File {
	pub name: &'static str,
	pub data: &'static [u8],
}
