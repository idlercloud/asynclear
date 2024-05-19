//! [`cervine::Cow`] is an [`alloc::borrow::Cow`] alternative that has different generic type constraints.
//!
//! [`cervine::Cow`]: enum.Cow.html
//! [`alloc::borrow::Cow`]: https://doc.rust-lang.org/stable/alloc/borrow/enum.Cow.html
//!
//! # Features
//!
//! `"serde"`: Implements [`serde::Deserialize`] and [`serde::Serialize`] on [`Cow`].
//!
//! [`serde::Deserialize`]: https://docs.rs/serde/1.0.115/serde/trait.Deserialize.html
//! [`serde::Serialize`]: https://docs.rs/serde/1.0.115/serde/trait.Serialize.html
//! [`Cow`]: enum.Cow.html
//!
//! # Examples
//!
//! Same type (`T = R = [bool; 2]`):
//!
//! ```rust
//! use cervine::Cow;
//! use rand::prelude::*;
//! use std::borrow::Borrow as _;
//!
//! let data = [true, false];
//! let mut cow = Cow::Borrowed(&data);
//!
//! if thread_rng().gen() {
//!   cow = Cow::Owned([false, true]);
//! }
//!
//! let array_ref: &[bool; 2] = cow.borrow();
//! ```
//!
//! Different types (`T = String` and `R = str`):
//!
//! ```rust
//! use cervine::Cow;
//! use rand::prelude::*;
//! use smartstring::alias::String;
//!
//! let mut cow = Cow::Borrowed("borrowed");
//!
//! if thread_rng().gen() {
//!   cow = Cow::Owned(String::from("owned"));
//! }
//!
//! let str_ref: &str = cow.as_ref();
//! ```

#![no_std]
#![warn(clippy::pedantic)]
#![doc(html_root_url = "https://docs.rs/cervine/0.0.6")]

#[cfg(doctest)]
pub mod readme {
	doc_comment::doctest!("../README.md");
}

use core::{
	borrow::Borrow,
	cmp::Ordering,
	convert::{TryFrom, TryInto as _},
	fmt::{self, Display, Formatter},
	hash::{Hash, Hasher},
	ops::Deref,
};

#[cfg(feature = "serde")]
use serde::{de, ser};

/// [`Cow`] is a clone-on-write smart pointer largely analogous to [`alloc::borrow::Cow`], with one key difference:  
/// Instead of requiring [`ToOwned`], the owned and borrowed type are both configurable and most methods require [`T: Borrow<R>`].
///
/// [`serde::Deserialize`] and [`serde::Serialize`] are only available with the `"serde"` feature.
///
/// [`Cow`]: enum.Cow.html
/// [`alloc::borrow::Cow`]: https://doc.rust-lang.org/stable/alloc/borrow/enum.Cow.html
/// [`ToOwned`]: https://doc.rust-lang.org/stable/alloc/borrow/trait.ToOwned.html
/// [`T: Borrow<R>`]: https://doc.rust-lang.org/stable/alloc/borrow/trait.Borrow.html
///
/// [`serde::Deserialize`]: https://docs.rs/serde/1.0.115/serde/trait.Deserialize.html
/// [`serde::Serialize`]: https://docs.rs/serde/1.0.115/serde/trait.Serialize.html
#[derive(Debug)]
pub enum Cow<'a, T, R: ?Sized> {
	Owned(T),
	Borrowed(&'a R),
}

impl<'a, T: Borrow<R>, R: ?Sized> Cow<'a, T, R> {
	/// Returns whether this value is a borrowed variant.
	///
	/// # Example
	///
	/// ```rust
	/// use cervine::Cow;
	///
	/// let borrowed: Cow<String, _> = Cow::Borrowed("borrowed");
	/// let owned: Cow<_, str> = Cow::Owned("owned".to_string());
	///
	/// assert!(borrowed.is_borrowed());
	/// assert!(!owned.is_borrowed());
	/// ```
	pub fn is_borrowed(&self) -> bool {
		matches!(self, Cow::Borrowed(_))
	}

	/// Returns whether this value is an owned variant.
	///
	/// # Example
	///
	/// ```rust
	/// use cervine::Cow;
	///
	/// let borrowed: Cow<String, _> = Cow::Borrowed("borrowed");
	/// let owned: Cow<_, str> = Cow::Owned("owned".to_string());
	///
	/// assert!(!borrowed.is_owned());
	/// assert!(owned.is_owned());
	/// ```
	pub fn is_owned(&self) -> bool {
		matches!(self, Cow::Owned(_))
	}
}

impl<'a, T: From<&'a R>, R: ?Sized> Cow<'a, T, R> {
	/// Converts this value into `T`.
	///
	/// # Example
	///
	/// ```rust
	/// use cervine::Cow;
	///
	/// let borrowed = Cow::Borrowed("borrowed");
	/// let owned: Cow<_, str> = Cow::Owned("owned".into());
	///
	/// let not_borrowed: String = borrowed.into_owned(); // Clones the `String`.
	/// let owned: String = owned.into_owned(); // Moves the `String`.
	/// ```
	pub fn into_owned(self) -> T {
		match self {
			Cow::Owned(t) => t,
			Cow::Borrowed(r) => r.into(),
		}
	}

	/// Retrieves a mutable reference to the contained `T`.
	///
	/// If this value is a borrowed variant, it is converted in place into an owned variant first.
	///
	/// # Example
	///
	/// ```rust
	/// use cervine::Cow;
	///
	/// let mut borrowed = Cow::Borrowed("borrowed");
	/// let mut owned: Cow<_, str> = Cow::Owned("owned".into());
	///
	/// let mutable_copy: &mut String = borrowed.make_mut(); // Clones the `String`.
	/// let mutable_borrow: &mut String = owned.make_mut();
	/// ```
	pub fn make_mut(&mut self) -> &mut T {
		match self {
			Cow::Owned(t) => t,
			Cow::Borrowed(r) => {
				*self = Cow::Owned((*r).into());
				match self {
					Cow::Owned(t) => t,
					Cow::Borrowed(_) => unreachable!(),
				}
			}
		}
	}
}

impl<'a, T: TryFrom<&'a R>, R: ?Sized> Cow<'a, T, R> {
	/// Converts this value into `T` if possible.
	///
	/// # Example
	///
	/// ```rust
	/// use cervine::Cow;
	///
	/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// let borrowed = Cow::Borrowed("borrowed");
	/// let owned: Cow<_, str> = Cow::Owned("owned".into());
	///
	/// let not_borrowed: String = borrowed.try_into_owned()?; // Clones the `String`.
	/// let owned: String = owned.try_into_owned()?; // Moves the `String`.
	/// # Ok(()) }
	/// ```
	///
	/// # Errors
	///
	/// * [`<T as TryFrom<&'a R>>::Error`]: Returned iff this value is a borrowed variant and conversion into `T` fails.
	///
	/// [`<T as TryFrom<&'a R>>::Error`]: https://doc.rust-lang.org/stable/core/convert/trait.TryFrom.html#associatedtype.Error
	pub fn try_into_owned(self) -> Result<T, T::Error> {
		match self {
			Cow::Owned(t) => Ok(t),
			Cow::Borrowed(r) => r.try_into(),
		}
	}

	/// Retrieves a mutable reference to the contained `T`, if possible.
	///
	/// If this value is a borrowed variant, an attempt is made to convert it in place into an owned variant first.
	///
	/// # Example
	///
	/// ```rust
	/// use cervine::Cow;
	///
	/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// let mut borrowed = Cow::Borrowed("borrowed");
	/// let mut owned: Cow<_, str> = Cow::Owned("owned".into());
	///
	/// let mutable_copy: &mut String = borrowed.try_make_mut()?; // Clones the `String`.
	/// let mutable_borrow: &mut String = owned.try_make_mut()?; // Moves the `String`.
	/// # Ok(()) }
	/// ```
	///
	/// # Errors
	///
	/// * [`<T as TryFrom<&'a R>>::Error`]: Returned iff this value is a borrowed variant and conversion into the owned variant fails.
	///
	/// [`<T as TryFrom<&'a R>>::Error`]: https://doc.rust-lang.org/stable/core/convert/trait.TryFrom.html#associatedtype.Error
	pub fn try_make_mut(&mut self) -> Result<&mut T, T::Error> {
		match self {
			Cow::Owned(t) => Ok(t),
			Cow::Borrowed(r) => {
				*self = Cow::Owned((*r).try_into()?);
				match self {
					Cow::Owned(t) => Ok(t),
					Cow::Borrowed(_) => unreachable!(),
				}
			}
		}
	}
}

impl<'a, T: AsRef<R>, R: ?Sized> AsRef<R> for Cow<'a, T, R> {
	fn as_ref(&self) -> &R {
		match self {
			Cow::Owned(t) => t.as_ref(),
			Cow::Borrowed(r) => r,
		}
	}
}

impl<'a, T: Deref<Target = R>, R: ?Sized> Deref for Cow<'a, T, R> {
	type Target = R;
	fn deref(&self) -> &Self::Target {
		match self {
			Cow::Owned(t) => t,
			Cow::Borrowed(r) => r,
		}
	}
}

impl<'a, T: Borrow<R>, R: ?Sized> Borrow<R> for Cow<'a, T, R> {
	fn borrow(&self) -> &R {
		match self {
			Cow::Owned(t) => t.borrow(),
			Cow::Borrowed(r) => r,
		}
	}
}

impl<'a, T: Borrow<R>, R: Display + ?Sized> Display for Cow<'a, T, R> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		Borrow::<R>::borrow(self).fmt(f)
	}
}

impl<'a, T: Copy, R: ?Sized> Copy for Cow<'a, T, R> {}

impl<'a, T: Clone, R: ?Sized> Clone for Cow<'a, T, R> {
	fn clone(&self) -> Self {
		match self {
			Cow::Owned(t) => Self::Owned(t.clone()),
			Cow::Borrowed(r) => Self::Borrowed(r),
		}
	}
}

impl<'a, T: Default, R: ?Sized> Default for Cow<'a, T, R> {
	fn default() -> Self {
		Cow::Owned(T::default())
	}
}

impl<'a, T: Borrow<R>, R: PartialEq + ?Sized> PartialEq<R> for Cow<'a, T, R> {
	fn eq(&self, other: &R) -> bool {
		Borrow::<R>::borrow(self) == other
	}
}

impl<'a, T: Borrow<R>, R: PartialEq + ?Sized> PartialEq for Cow<'a, T, R> {
	fn eq(&self, other: &Self) -> bool {
		Borrow::<R>::borrow(self) == other.borrow()
	}
}

impl<'a, T: Borrow<R>, R: Eq + ?Sized> Eq for Cow<'a, T, R> {}

impl<'a, T: Borrow<R>, R: PartialOrd + ?Sized> PartialOrd<R> for Cow<'a, T, R> {
	fn partial_cmp(&self, other: &R) -> Option<Ordering> {
		Borrow::<R>::borrow(self).partial_cmp(other)
	}
	fn lt(&self, other: &R) -> bool {
		Borrow::<R>::borrow(self).lt(other)
	}
	fn le(&self, other: &R) -> bool {
		Borrow::<R>::borrow(self).le(other)
	}
	fn gt(&self, other: &R) -> bool {
		Borrow::<R>::borrow(self).gt(other)
	}
	fn ge(&self, other: &R) -> bool {
		Borrow::<R>::borrow(self).ge(other)
	}
}

impl<'a, T: Borrow<R>, R: PartialOrd + ?Sized> PartialOrd for Cow<'a, T, R> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Borrow::<R>::borrow(self).partial_cmp(other.borrow())
	}
	fn lt(&self, other: &Self) -> bool {
		Borrow::<R>::borrow(self).lt(other.borrow())
	}
	fn le(&self, other: &Self) -> bool {
		Borrow::<R>::borrow(self).le(other.borrow())
	}
	fn gt(&self, other: &Self) -> bool {
		Borrow::<R>::borrow(self).gt(other.borrow())
	}
	fn ge(&self, other: &Self) -> bool {
		Borrow::<R>::borrow(self).ge(other.borrow())
	}
}

impl<'a, T: Borrow<R>, R: Ord + ?Sized> Ord for Cow<'a, T, R> {
	fn cmp(&self, other: &Self) -> Ordering {
		Borrow::<R>::borrow(self).cmp(other.borrow())
	}

	// min, max and clamp handled by default implementation, since they can't be forwarded directly.
}

impl<'a, T: Borrow<R>, R: Hash + ?Sized> Hash for Cow<'a, T, R> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		Borrow::<R>::borrow(self).hash(state)
	}
}

/// Requires `"serde"` feature.
#[cfg(feature = "serde")]
impl<'a, T: Borrow<R>, R: ser::Serialize + ?Sized> ser::Serialize for Cow<'a, T, R> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: ser::Serializer,
	{
		Borrow::<R>::borrow(self).serialize(serializer)
	}
}

/// Requires `"serde"` feature.
#[cfg(feature = "serde")]
impl<'a, 'de, T: de::Deserialize<'de>, R: ?Sized> de::Deserialize<'de> for Cow<'a, T, R> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		T::deserialize(deserializer).map(Cow::Owned)
	}

	fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		match place {
			Cow::Owned(t) => T::deserialize_in_place(deserializer, t),
			Cow::Borrowed(_) => serde::Deserialize::deserialize(deserializer).map(|de| *place = de),
		}
	}
}
