use std::path::PathBuf;

use crate::note::MidiNote;

#[derive(Debug)]
pub enum BuzzerMessage {
	Clear,
	Note { on: bool, note: MidiNote },
}

#[derive(Debug)]
pub enum ButtonMessage {
	Press(u8),
	Release(u8),
}

#[derive(Debug)]
pub enum PlayerMessage {
	Play(PathBuf),
	Stop,
}

#[derive(Debug)]
pub enum AlphanumMessage {
	Static([char; 4]),
	Text(String),
	Time,
	Empty,
}

#[derive(Debug)]
pub enum SongEventMessage {
	Start(String),
	End(String),
}
