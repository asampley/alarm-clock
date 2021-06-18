use std::fs;
use std::sync::mpsc;
use std::sync::RwLock;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{ Instant, Duration };

use once_cell::sync::Lazy;

mod circuit;
use circuit::Buzzer;
use circuit::Button;
use circuit::Alphanum;

mod config;
use config::Config;

mod message;
use message::{AlphanumMessage, PlayerMessage, EventMessage, ButtonEvent, SongEvent};

mod note;
use note::MidiNote;

mod selector;
use selector::{LinearSelector, Selector};

mod threads;
use threads::input::poll_inputs;
use threads::buzzer::update_buzzer;
use threads::player::midi_player;
use threads::alphanum::alphanum_thread;

#[cfg(test)] mod tests;

static TIME_ZERO: Lazy<RwLock<Instant>> = Lazy::new(|| RwLock::new(Instant::now()));

#[derive(Debug)]
enum Error {
	Gpio(rppal::gpio::Error),
	I2c(rppal::i2c::Error),
}

impl From<rppal::gpio::Error> for Error {
	fn from(from: rppal::gpio::Error) -> Self {
		Self::Gpio(from)
	}
}

impl From<rppal::i2c::Error> for Error {
	fn from(from: rppal::i2c::Error) -> Self {
		Self::I2c(from)
	}
}

fn main() -> Result<(), Error> {
	let mut args = std::env::args();

	// parse arguments
	if 2 != args.len() {
		eprintln!("Usage: {} CONFIG", args.next().unwrap());
		return Ok(());
	}

	let args = args.collect::<Vec<_>>();

	Lazy::force(&TIME_ZERO);

	let config: Config = toml::from_str(
			&fs::read_to_string(&args[1])
				.expect(&format!("Unable to read config file \"{}\"", &args[1]))
		).expect("Unable to parse config file");

	// create buzzer controller
	let buzzer = Buzzer::<MidiNote>::new(config.buzzer_pin)?;

	// create button pollers
	let buttons = config.button_pins.iter()
		.map(|&p| Button::new(p).map_err(|e| e.into()))
		.collect::<Result<Vec<_>, Error>>()?;

	// create alphanum controller
	let mut alphanum = Alphanum::new()?;
	alphanum.set_brightness(config.brightness)?;

	// create channels for messages
	let (midi_note_sender, midi_note_receiver) = mpsc::channel();
	let (event_sender, event_receiver) = mpsc::channel();
	let (player_sender, player_receiver) = mpsc::channel();
	let (alphanum_sender, alphanum_receiver) = mpsc::channel();

	// start thread to update buzzer
	let _thread_buzzer = thread::spawn(move || update_buzzer(midi_note_receiver, buzzer));

	// start thread to read poll button
	let _thread_buttons = {
		let event_sender_2 = event_sender.clone();
		thread::spawn(move || { poll_inputs(event_sender_2, buttons) })
	};

	// start playing midi file
	let _thread_midi_player = thread::spawn(move ||
		midi_player(player_receiver, midi_note_sender, event_sender)
	);

	// start display thread
	let _thread_display = {
		let scroll_delay = Duration::from_millis(config.scroll_delay_ms);
		thread::spawn(move ||
			alphanum_thread(alphanum, alphanum_receiver, scroll_delay)
		)
	};

	let mut midi_files = list_files(&config.midi_dir)
		.expect(&format!("Unable to read the directory \"{:?}\"", config.midi_dir))
		.collect::<Vec<PathBuf>>();
	midi_files.sort();

	let mut selector = LinearSelector::new(midi_files);

	loop {
		match event_receiver.recv() {
			Ok(msg) => match msg {
				EventMessage::Button(button) => match button {
					ButtonEvent::Press(x) => {
						if x == 0 || x == 1 {
							let midi_file = if x == 0 {
								selector.incr()
							} else {
								selector.decr()
							};

							player_sender.send(PlayerMessage::Play(midi_file.clone()))
								.expect("Unable to send midi file name");
						}
					}
					ButtonEvent::Release(_) => (),
				}
				EventMessage::Song(song) => match song {
					SongEvent::Start(name) => {
						println!("Now playing {:?}", name);
						alphanum_sender.send(AlphanumMessage::Text(name)).unwrap();
					}
					SongEvent::End(name) => {
						println!("Stopped playing {:?}", name);
						alphanum_sender.send(AlphanumMessage::Time).unwrap();
					}
				}
			}
			Err(_) => break,
		}
	}

	//println!("{} updates per second", thread_buzzer.join().unwrap()?);
	//thread_midi_player.join().unwrap();
	//thread_buttons.join().unwrap()?;

	Ok(())
}

fn list_files<P: AsRef<Path>>(dir: P) -> std::io::Result<impl Iterator<Item = PathBuf>> {
	Ok(fs::read_dir(dir)?
		.filter(|f| f.is_ok())
		.map(|f| f.unwrap().path())
		.peekable()
	)
}
