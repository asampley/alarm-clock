use std::cmp::min;

pub trait Selector<T> {
	fn incr(&mut self) -> &T;
	fn decr(&mut self) -> &T;
	fn curr(&self) -> &T;
	fn reset(&mut self);
	fn len(&self) -> usize;
}

pub struct LinearSelector<T> {
	list: Vec<T>,
	curr: usize,
}

impl <T> LinearSelector<T> {
	pub fn new(list: Vec<T>) -> Self {
		Self { list, curr: 0 }
	}
}

impl <T> Selector<T> for LinearSelector<T> {
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

enum Bound {
	Single(usize),
	Range(usize, usize)
}

pub struct BinarySelector<T> {
	list: Vec<T>,
	bound: Bound,
}

impl <T> BinarySelector<T> {
	pub fn new(list: Vec<T>) -> Self {
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

impl <T> Selector<T> for BinarySelector<T> {
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
