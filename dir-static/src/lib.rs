#![no_std]

pub use dir_static_macro::*;

#[derive(Debug, Copy, Clone)]
pub struct File {
	pub name: &'static str,
	pub data: &'static [u8],
}
