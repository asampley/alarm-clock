use core::fmt::Debug;
use core::ops::Deref;

pub enum Calf<'a, T: Deref> {
	Borrowed(&'a <T as Deref>::Target),
	Owned(T),
}

impl<T: Deref> Deref for Calf<'_, T> {
	type Target = T::Target;

	fn deref(&self) -> &Self::Target {
		match self {
			Self::Borrowed(b) => b,
			Self::Owned(o) => o.deref(),
		}
	}
}

impl<T> Debug for Calf<'_, T>
where
	T: Debug + Deref,
	<T as Deref>::Target: Debug,
{
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::Borrowed(v) => {
				write!(f, "Borrowed(")?;
				v.fmt(f)?;
				write!(f, ")")?;
			}
			Self::Owned(v) => {
				write!(f, "Owned(")?;
				v.fmt(f)?;
				write!(f, ")")?;
			}
		}

		Ok(())
	}
}
