#![cfg(test)]

use crate::MidiNote;
use crate::note::Note;
use crate::circuit::{Buzzer};
use crate::selector::{BinarySelector, LinearSelector, Selector};

use std::{thread, time};

macro_rules! assert_delta {
	($x:expr, $y:expr, $d:expr) => {
		assert!(($x - $y).abs() < $d)
	}
}

#[test]
fn test_freqencies() {
	let delta = 1e-2;

	assert_delta!(dbg!(MidiNote(12).frequency()), 16.35, delta);
	assert_delta!(dbg!(MidiNote(33).frequency()), 55.00, delta);
	assert_delta!(dbg!(MidiNote(73).frequency()), 554.37, delta);
	assert_delta!(dbg!(MidiNote(55).frequency()), 196.00, delta);
	assert_delta!(dbg!(MidiNote(4).frequency() ), 10.30, delta);
	assert_delta!(dbg!(MidiNote(-1).frequency()), 7.71, delta);
}

/// qualitative test only
/// uses pin 12
#[test] #[ignore]
fn test_range() -> rppal::gpio::Result<()> {
	let mut buzzer = Buzzer::new(12)?;
	let dur = time::Duration::from_millis(500);

	for note in 60..72 {
		buzzer.add_note(MidiNote(note));

		let time = time::Instant::now();
		while time.elapsed() < dur {
			buzzer.update();
		}

		buzzer.remove_note(&MidiNote(note));
	}

	Ok(())
}

#[test]
fn test_linear_selector() {
	let even = vec![0, 1, 2, 3];
	let odd = vec![0, 1, 2, 3, 4];

	let mut selector_even = LinearSelector::new(even);
	let mut selector_odd = LinearSelector::new(odd);

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

#[test]
fn test_binary_selector() {
	let even = (0..10_usize).collect();
	let odd = (0..11_usize).collect();

	let mut selector_even = BinarySelector::new(even);
	let mut selector_odd = BinarySelector::new(odd);

	assert_eq!(selector_even.curr(), &5);
	assert_eq!(selector_odd.curr(), &5);

	// test increment with wrap
	for x in &[8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0] {
		assert_eq!(selector_even.incr(), x);
	}

	for x in &[8, 10, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0] {
		assert_eq!(selector_odd.incr(), x);
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
