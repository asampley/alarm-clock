pub trait Selector<T> {
	fn incr(&mut self) -> &T;
	fn decr(&mut self) -> &T;
	fn selected(&self) -> &T;
}

pub struct LinearSelector<T> {
	list: Vec<T>,
	selected: usize,
}

impl <T> LinearSelector<T> {
	pub fn new(list: Vec<T>) -> Self {
		Self { list, selected: 0 }
	}
}

impl <T> Selector<T> for LinearSelector<T> {
	fn incr(&mut self) -> &T {
		self.selected = (self.selected + 1) % self.list.len();
		self.selected()
	}

	fn decr(&mut self) -> &T {
		self.selected = match self.selected {
			0 => self.list.len() - 1,
			_ => self.selected - 1,
		};
		self.selected()
	}

	fn selected(&self) -> &T {
		&self.list[self.selected]
	}
}
