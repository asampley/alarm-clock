use core::cmp::min;

pub trait Selector<T> {
	fn incr(&mut self) -> &T;
	fn decr(&mut self) -> &T;
	fn curr(&self) -> &T;
	#[allow(dead_code)]
	fn reset(&mut self);
	fn len(&self) -> usize;
}

#[derive(Debug)]
pub struct LinearSelector<'a, T> {
	list: &'a [T],
	curr: usize,
}

impl<'a, T> LinearSelector<'a, T> {
	pub fn new(list: &'a [T]) -> Self {
		Self { list, curr: 0 }
	}
}

impl<T> Selector<T> for LinearSelector<'_, T> {
	fn incr(&mut self) -> &T {
		self.curr = (self.curr + 1) % self.list.len();
		self.curr()
	}

	fn decr(&mut self) -> &T {
		self.curr = match self.curr {
			0 => self.list.len() - 1,
			_ => self.curr - 1,
		};
		self.curr()
	}

	fn curr(&self) -> &T {
		&self.list[self.curr]
	}

	fn reset(&mut self) {
		self.curr = 0;
	}

	fn len(&self) -> usize {
		self.list.len()
	}
}

#[derive(Debug)]
enum Bound {
	Single(usize),
	Range(usize, usize),
}

#[derive(Debug)]
pub struct BinarySelector<'a, T> {
	list: &'a [T],
	bound: Bound,
}

impl<'a, T> BinarySelector<'a, T> {
	pub fn new(list: &'a [T]) -> Self {
		let bound = Bound::Range(0, list.len() - 1);
		Self { list, bound }
	}

	fn curr_i(&self) -> usize {
		match self.bound {
			Bound::Single(i) => i,
			Bound::Range(i, j) => i + (j - i + 1) / 2,
		}
	}
}

impl<T> Selector<T> for BinarySelector<'_, T> {
	fn incr(&mut self) -> &T {
		self.bound = match self.bound {
			Bound::Single(i) => Bound::Single((i + 1) % self.len()),
			Bound::Range(_, j) => {
				let new = self.curr_i() + 1;

				if new >= j {
					Bound::Single(new % self.len())
				} else {
					Bound::Range(new, j)
				}
			}
		};

		self.curr()
	}

	fn decr(&mut self) -> &T {
		self.bound = match self.bound {
			Bound::Single(i) => Bound::Single(min(self.len() - 1, i.wrapping_sub(1))),
			Bound::Range(i, _) => {
				let new = min(self.len() - 1, self.curr_i().wrapping_sub(1));

				if new <= i {
					Bound::Single(new)
				} else {
					Bound::Range(i, new)
				}
			}
		};

		self.curr()
	}

	fn curr(&self) -> &T {
		&self.list[self.curr_i()]
	}

	fn reset(&mut self) {
		self.bound = Bound::Range(0, self.len() - 1);
	}

	fn len(&self) -> usize {
		self.list.len()
	}
}

#[derive(Debug)]
pub enum ExponentialSelector {
	Increasing { value: usize },
	Tuning { value: usize, digit: u8 },
	Single { value: usize },
}

impl Default for ExponentialSelector {
	fn default() -> Self {
		Self::Increasing { value: 1 }
	}
}

impl Selector<usize> for ExponentialSelector {
	fn incr(&mut self) -> &usize {
		match self {
			Self::Increasing { value } => {
				if let Some(new) = value.checked_shl(1) {
					*value = new;
				}
			}
			Self::Tuning { value, digit } => {
				if *digit == 0 {
					*self = Self::Single { value: *value };
					self.incr();
				} else {
					*digit -= 1;
					*value |= 1 << *digit;
				}
			}
			Self::Single { value } => *value = value.saturating_add(1),
		}

		self.curr()
	}

	fn decr(&mut self) -> &usize {
		match self {
			Self::Increasing { value } => {
				*value >>= 1;

				if *value < 2 {
					*self = Self::Single { value: *value }
				} else {
					let digit = value.ilog2() - 1;

					*value |= 1 << digit;

					*self = Self::Tuning {
						value: *value,
						digit: digit as u8,
					};
				}
			}
			Self::Tuning { value, digit } => {
				if *digit == 0 {
					*self = Self::Single { value: *value };
					self.decr();
				} else {
					*value &= !(1 << *digit);
					*digit -= 1;
					*value |= 1 << *digit;
				}
			}
			Self::Single { value } => {
				*value = value.saturating_sub(1);
			}
		}

		self.curr()
	}

	fn curr(&self) -> &usize {
		match self {
			Self::Increasing { value } | Self::Tuning { value, .. } | Self::Single { value } => {
				value
			}
		}
	}

	fn reset(&mut self) {
		*self = Self::Increasing { value: 0 }
	}

	fn len(&self) -> usize {
		usize::MAX
	}
}
