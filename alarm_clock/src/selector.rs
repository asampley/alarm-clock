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

pub struct BinarySelector<T> {
	list: Vec<T>,
	bound: (usize, usize),
}

impl <T> BinarySelector<T> {
	pub fn new(list: Vec<T>) -> Self {
		let bound = (0, list.len() - 1);
		Self { list, bound }
	}

	fn curr_i(&self) -> usize {
		self.bound.0 + (self.bound.1 - self.bound.0 + 1) / 2
	}
}

impl <T> Selector<T> for BinarySelector<T> {
	fn incr(&mut self) -> &T {
		if self.bound.0 == self.bound.1 {
			self.bound.0 = (self.bound.1 + 1) % self.len();
			self.bound.1 = self.bound.0;
		} else {
			self.bound.0 = self.curr_i() + 1;

			if self.bound.0 >= self.len() {
				self.bound.0 = 0;
				self.bound.1 = 0;
			}
		}

		self.curr()
	}

	fn decr(&mut self) -> &T {
		if self.bound.0 >= self.bound.1 {
			self.bound.0 = min(self.len() - 1, self.bound.0.wrapping_sub(1));
			self.bound.1 = self.bound.0;
		} else {
			self.bound.1 = min(self.len() - 1, self.curr_i().wrapping_sub(1));
		}

		self.curr()
	}

	fn curr(&self) -> &T {
		dbg!((self.bound, self.curr_i()));
		&self.list[self.curr_i()]
	}

	fn reset(&mut self) {
		self.bound = (0, self.list.len() - 1);
	}

	fn len(&self) -> usize {
		self.list.len()
	}
}
