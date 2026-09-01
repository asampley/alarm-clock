use core::fmt::Debug;
use core::ops::Deref;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Either<T, U> {
	First(T),
	Second(U),
}

impl<T, U> Debug for Either<T, U>
where
	T: Debug,
	U: Debug,
{
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::First(v) => {
				write!(f, "First(")?;
				v.fmt(f)?;
				write!(f, ")")?;
			}
			Self::Second(v) => {
				write!(f, "Second(")?;
				v.fmt(f)?;
				write!(f, ")")?;
			}
		}

		Ok(())
	}
}
