//! A vector that is guaranteed to hold at least one element.
//!
//! This is the Rust counterpart of Cats' `NonEmptyChain`, used by
//! [`Decision`](crate::Decision) for accepted events and rejection reasons.

use std::ops::Deref;

/// A non-empty, ordered collection.
///
/// `NonEmpty<T>` dereferences to `[T]`, so every read-only slice method
/// (`iter`, `len`, `first`, `last`, ...) is available.
///
/// ```
/// use edomata_core::{NonEmpty, nonempty};
///
/// let ne = nonempty![1, 2, 3];
/// assert_eq!(ne.head(), &1);
/// assert_eq!(ne.tail(), &[2, 3]);
/// assert_eq!(ne.len(), 3);
/// assert!(NonEmpty::<i32>::from_vec(vec![]).is_none());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NonEmpty<T> {
    items: Vec<T>,
}

#[cfg(feature = "serde")]
impl<T: serde::Serialize> serde::Serialize for NonEmpty<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.items.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for NonEmpty<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let items = Vec::<T>::deserialize(deserializer)?;
        Self::from_vec(items).ok_or_else(|| serde::de::Error::custom(EmptyError))
    }
}

impl<T> NonEmpty<T> {
    /// Creates a collection holding a single element.
    pub fn new(head: T) -> Self {
        Self { items: vec![head] }
    }

    /// Creates a collection from a head and any number of trailing elements.
    pub fn of<I: IntoIterator<Item = T>>(head: T, tail: I) -> Self {
        let mut items = vec![head];
        items.extend(tail);
        Self { items }
    }

    /// Creates a collection from a vector, returning `None` when it is empty.
    pub fn from_vec(items: Vec<T>) -> Option<Self> {
        if items.is_empty() {
            None
        } else {
            Some(Self { items })
        }
    }

    /// Collects an iterator, returning `None` when it yields nothing.
    pub fn try_from_iter<I: IntoIterator<Item = T>>(iter: I) -> Option<Self> {
        Self::from_vec(iter.into_iter().collect())
    }

    /// The first element.
    pub fn head(&self) -> &T {
        &self.items[0]
    }

    /// Every element after the first.
    pub fn tail(&self) -> &[T] {
        &self.items[1..]
    }

    /// Appends one element at the end.
    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    /// Appends all elements of `other`, preserving order.
    pub fn append(&mut self, other: NonEmpty<T>) {
        self.items.extend(other.items);
    }

    /// Prepends all elements of `other`, so that they come first.
    pub fn prepend<I: IntoIterator<Item = T>>(&mut self, other: I) {
        let mut items: Vec<T> = other.into_iter().collect();
        items.append(&mut self.items);
        self.items = items;
    }

    /// Concatenates two collections into a new one.
    pub fn concat(mut self, other: NonEmpty<T>) -> Self {
        self.append(other);
        self
    }

    /// Consumes the collection and returns the underlying vector.
    pub fn into_vec(self) -> Vec<T> {
        self.items
    }

    /// Borrows the underlying elements as a slice.
    pub fn as_slice(&self) -> &[T] {
        &self.items
    }

    /// Applies `f` to every element.
    pub fn map<B, F: FnMut(T) -> B>(self, f: F) -> NonEmpty<B> {
        NonEmpty {
            items: self.items.into_iter().map(f).collect(),
        }
    }

    /// Returns the last element.
    pub fn last(&self) -> &T {
        self.items.last().expect("NonEmpty is never empty")
    }

    /// Always `false`; provided for API symmetry with `Vec`.
    pub fn is_empty(&self) -> bool {
        false
    }
}

impl<T> Deref for NonEmpty<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.items
    }
}

impl<T> AsRef<[T]> for NonEmpty<T> {
    fn as_ref(&self) -> &[T] {
        &self.items
    }
}

impl<T> From<T> for NonEmpty<T> {
    fn from(head: T) -> Self {
        Self::new(head)
    }
}

impl<T> TryFrom<Vec<T>> for NonEmpty<T> {
    type Error = EmptyError;

    fn try_from(items: Vec<T>) -> Result<Self, EmptyError> {
        Self::from_vec(items).ok_or(EmptyError)
    }
}

impl<T> From<NonEmpty<T>> for Vec<T> {
    fn from(ne: NonEmpty<T>) -> Vec<T> {
        ne.items
    }
}

impl<T> Extend<T> for NonEmpty<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.items.extend(iter);
    }
}

impl<T> IntoIterator for NonEmpty<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a NonEmpty<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

/// Error returned when trying to build a [`NonEmpty`] from an empty vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmptyError;

impl std::fmt::Display for EmptyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("collection must not be empty")
    }
}

impl std::error::Error for EmptyError {}

/// Builds a [`NonEmpty`] from one or more expressions.
///
/// ```
/// use edomata_core::nonempty;
///
/// let ne = nonempty!["a", "b"];
/// assert_eq!(ne.len(), 2);
/// ```
#[macro_export]
macro_rules! nonempty {
    ($head:expr $(, $tail:expr)* $(,)?) => {
        $crate::NonEmpty::of($head, [$($tail),*])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_and_tail() {
        let ne = nonempty![1, 2, 3];
        assert_eq!(ne.head(), &1);
        assert_eq!(ne.tail(), &[2, 3]);
        assert_eq!(ne.last(), &3);
    }

    #[test]
    fn concat_preserves_order() {
        let a = nonempty![1, 2];
        let b = nonempty![3];
        assert_eq!(a.concat(b).into_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn prepend_puts_items_first() {
        let mut a = nonempty![3];
        a.prepend(vec![1, 2]);
        assert_eq!(a.into_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn empty_vec_is_rejected() {
        assert_eq!(NonEmpty::<u8>::try_from(vec![]), Err(EmptyError));
    }
}
