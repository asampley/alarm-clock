#![cfg(test)]

use crate::dbg;

use embassy_time::{Duration, Instant};
use heapless::Vec;

use crate::{ MidiNote, CONFIG };
use crate::note::Note;
use crate::circuit::{ Alphanum, Buzzer };
use crate::selector::{ BinarySelector, LinearSelector, Selector };

macro_rules! assert_delta {
	($x:expr, $y:expr, $d:expr) => {
		assert!(($x - $y) < $d && ($x - $y) > -$d)
	}
}

static _test_frequencies: () = test_freqencies();
static _test_linear_selector: () = test_linear_selector();
static _test_binary_selector: () = test_binary_selector();

const fn test_freqencies() {
	let delta = 1e-2;

	assert_delta!(MidiNote(12).frequency(), 16.35, delta);
	assert_delta!(MidiNote(33).frequency(), 55.00, delta);
	assert_delta!(MidiNote(73).frequency(), 554.37, delta);
	assert_delta!(MidiNote(55).frequency(), 196.00, delta);
	assert_delta!(MidiNote(4).frequency(), 10.30, delta);
	assert_delta!(MidiNote(-1).frequency(), 7.71, delta);
}

/// qualitative test only
#[test] #[ignore]
fn test_range() {
	let mut buzzer = Buzzer::new(CONFIG.get().lock().await.buzzer_pin());
	let dur = Duration::from_millis(500);

	for note in 60..72 {
		buzzer.add_note(MidiNote(note));

		let time = Instant::now();
		while time.elapsed() < dur {
			buzzer.update();
		}

		buzzer.remove_note(&MidiNote(note));
	}
}

/// qualitative test only
#[test] #[ignore]
fn test_alphanum() -> rppal::i2c::Result<()> {
	let mut alphanum = Alphanum::new()?;

	let string = "    !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~    ";
	let mut iter = string.chars();

	loop {
		let chars = iter.clone().take(4).collect::<Vec<_>>();

		if chars.len() != 4 {
			break;
		}

		alphanum.display_str(&chars.try_into().unwrap())?;
		iter.next();

		std::thread::sleep(time::Duration::from_millis(300));
	}

	Ok(())
}

fn test_linear_selector() {
	let even = [0, 1, 2, 3];
	let odd = [0, 1, 2, 3, 4];

	let mut selector_even = LinearSelector::new(&even);
	let mut selector_odd = LinearSelector::new(&odd);

	assert_eq!(selector_even.curr(), &0);
	assert_eq!(selector_odd.curr(), &0);

	for selector in &mut [&mut selector_even, &mut selector_odd] {
		// test increment through whole selector
		for i in 1..selector.len() {
			assert_eq!(selector.incr(), &i);
		}
		// test increment wrap
		for i in 0..selector.len() {
			assert_eq!(selector.incr(), &i);
		}

		// test decrement through whole selector
		for i in (0..selector.len() - 1).rev() {
			assert_eq!(selector.decr(), &i);
		}
		// test decrement wrap
		for i in (0..selector.len()).rev() {
			assert_eq!(selector.decr(), &i);
		}
	}

	// test reset
	selector_even.reset();
	selector_odd.reset();
	assert_eq!(selector_even.curr(), &0);
	assert_eq!(selector_odd.curr(), &0);
}

fn test_binary_selector() {
	let even: [usize; 10] = core::array::from_fn(|i| i);
	let odd: [usize; 11] = core::array::from_fn(|i| i);

	let mut selector_even = BinarySelector::new(&even);
	let mut selector_odd = BinarySelector::new(&odd);

	assert_eq!(selector_even.curr(), &5);
	assert_eq!(selector_odd.curr(), &5);

	// test increment with wrap
	for x in &[8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0] {
		assert_eq!(selector_even.incr(), x);
	}

	for x in &[8, 10, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0] {
		assert_eq!(dbg!(selector_odd.incr()), x);
	}

	// reset for next test
	selector_even.reset();
	selector_odd.reset();

	assert_eq!(selector_even.curr(), &5);
	assert_eq!(selector_odd.curr(), &5);

	// test decrement with wrap
	for x in &[2, 1, 0, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 9] {
		assert_eq!(selector_even.decr(), x);
	}

	for x in &[2, 1, 0, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 10] {
		assert_eq!(selector_odd.decr(), x);
	}

	// reset for next test
	selector_even.reset();
	selector_odd.reset();

	assert_eq!(selector_even.curr(), &5);
	assert_eq!(selector_odd.curr(), &5);

	// test forward and back
	for (f,x) in &[('d', 2), ('i', 4), ('d', 3), ('i', 4)] {
		match f {
			'i' => assert_eq!(selector_even.incr(), x),
			'd' => assert_eq!(selector_even.decr(), x),
			_ => unreachable!(),
		}
	}

	for (f,x) in &[('d', 2), ('i', 4), ('d', 3), ('i', 4)] {
		match f {
			'i' => assert_eq!(selector_odd.incr(), x),
			'd' => assert_eq!(selector_odd.decr(), x),
			_ => unreachable!(),
		}
	}
}
