use std::fs;
use std::sync::mpsc;
use std::sync::RwLock;
use std::path::{ Path, PathBuf };
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

mod note;
use note::MidiNote;

mod selector;

mod states;
use states::{
	State,
	state_time,
	state_mode_select,
	state_time_set,
	state_alarm_time_set,
	state_alarm_song_set,
	state_play,
};

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
	let (mut player_sender, player_receiver) = mpsc::channel();
	let (mut alphanum_sender, alphanum_receiver) = mpsc::channel();

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

	let mut state = State::Time;

	loop {
		println!("Entering state {:?}", state);

		let mut f = match state {
			State::Time => state_time(&mut alphanum_sender),
			State::ModeSelect => state_mode_select(&mut alphanum_sender),
			State::TimeSet => state_time_set(&mut alphanum_sender),
			State::AlarmTime => state_alarm_time_set(&mut alphanum_sender),
			State::AlarmSong => state_alarm_song_set(&midi_files, &mut alphanum_sender, &mut player_sender),
			State::Play => state_play(&midi_files, &mut alphanum_sender, &mut player_sender),
			State::Bad => break,
		};

		state = loop {
			match event_receiver.recv() {
				Ok(msg) => {
					if let Some(next_state) = f(msg) {
						break next_state;
					}
				}
				Err(_) => break State::Bad,
			}
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
