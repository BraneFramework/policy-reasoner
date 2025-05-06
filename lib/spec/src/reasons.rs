//  REASONS.rs
//    by Lut99
//
//  Created:
//    17 Oct 2024, 09:53:49
//  Last edited:
//    06 May 2025, 12:45:11
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines reasons for reasoner connectors calling a policy violated.
//

use std::fmt::{Display, Formatter, Result as FResult};
use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};


/***** LIBRARY ****/
/// Represents that no reason is used.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct NoReason;
impl Display for NoReason {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "<no reason>") }
}

/// Represents that multiple reasons can be given.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ManyReason<R>(Vec<R>);
impl<R> Default for ManyReason<R> {
    #[inline]
    fn default() -> Self { Self::new() }
}
impl<R> ManyReason<R> {
    /// Constructor for the ManyReason that initializes it as empty.
    ///
    /// # Returns
    /// A new ManyReason that doesn't have any reasons embedded in it yet.
    #[inline]
    pub fn new() -> Self { Self(Vec::new()) }

    /// Constructor for the ManyReason that initializes it as empty but with space allocated for
    /// a certain number of reasons.
    ///
    /// # Arguments
    /// - `capacity`: The (minimum) number of reasons to allocate space for.
    ///
    /// # Returns
    /// A new ManyReason that doesn't have any reasons embedded in it yet but space for at least `capacity` reasons.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self { Self(Vec::with_capacity(capacity)) }
}
impl<R: Display> Display for ManyReason<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        for i in 0..self.0.len() {
            if i > 0 && i < self.0.len() - 1 {
                write!(f, ", ")?;
            } else if i == self.0.len() {
                write!(f, " and ")?;
            }
            write!(f, "{}", self.0[i])?;
        }
        Ok(())
    }
}
impl<R> Deref for ManyReason<R> {
    type Target = Vec<R>;

    #[inline]
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<R> DerefMut for ManyReason<R> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}
impl<'a, R> IntoIterator for &'a ManyReason<R> {
    type IntoIter = std::slice::Iter<'a, R>;
    type Item = &'a R;

    #[inline]
    fn into_iter(self) -> Self::IntoIter { self.0.iter() }
}
impl<'a, R> IntoIterator for &'a mut ManyReason<R> {
    type IntoIter = std::slice::IterMut<'a, R>;
    type Item = &'a mut R;

    #[inline]
    fn into_iter(self) -> Self::IntoIter { self.0.iter_mut() }
}
impl<R> IntoIterator for ManyReason<R> {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = R;

    #[inline]
    fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}
impl<R> FromIterator<R> for ManyReason<R> {
    #[inline]
    fn from_iter<T: IntoIterator<Item = R>>(iter: T) -> Self { Self(iter.into_iter().collect()) }
}
