//  SPEC.rs
//    by Lut99
//
//  Created:
//    09 Oct 2024, 16:06:18
//  Last edited:
//    06 Nov 2024, 15:03:26
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines some general interface for this crate.
//

use std::borrow::Cow;
use std::convert::Infallible;
use std::error;

use eflint_json::spec::Phrase;
use thiserror::Error;


/***** ERRORS *****/
/// Special error that occurs when something cannot be serialized as eFLINT in a container of
/// those things.
#[derive(Debug, Error)]
#[error("Failed to serialize element {i} to eFLINT")]
pub struct Error<E> {
    /// The index of the failed element.
    i:   usize,
    /// The nested error.
    #[source]
    err: E,
}





/***** LIBRARY *****/
/// Defines something that can be turned into eFLINT phrases.
pub trait EFlintable {
    /// The error type returned when converting to eFLINT.
    type Error: error::Error;


    /// Converts this state to eFLINT phrases.
    ///
    /// # Returns
    /// A list of [`Phrase`]s that represent the eFLINT to send to the reasoner.
    ///
    /// # Errors
    /// This function can fail if `self` is not in a right state to be serialized to eFLINT.
    fn to_eflint(&self) -> Result<Vec<Phrase>, Self::Error>;
}

// Practical impls
impl EFlintable for () {
    type Error = Infallible;

    #[inline]
    fn to_eflint(&self) -> Result<Vec<Phrase>, Self::Error> { Ok(Vec::new()) }
}

// eFLINT impls
impl EFlintable for Phrase {
    type Error = Infallible;

    #[inline]
    fn to_eflint(&self) -> Result<Vec<Phrase>, Self::Error> { Ok(vec![self.clone()]) }
}

// Pointer impls
impl<'a, T: ?Sized + EFlintable> EFlintable for &'a T {
    type Error = T::Error;

    #[inline]
    fn to_eflint(&self) -> Result<Vec<Phrase>, Self::Error> { <T as EFlintable>::to_eflint(self) }
}
impl<'a, T: ?Sized + EFlintable> EFlintable for &'a mut T {
    type Error = T::Error;

    #[inline]
    fn to_eflint(&self) -> Result<Vec<Phrase>, Self::Error> { <T as EFlintable>::to_eflint(self) }
}
impl<'a, T: ?Sized + EFlintable + ToOwned> EFlintable for Cow<'a, T> {
    type Error = T::Error;

    #[inline]
    fn to_eflint(&self) -> Result<Vec<Phrase>, Self::Error> { <T as EFlintable>::to_eflint(self) }
}

// Container impls
impl<T: EFlintable> EFlintable for [T]
where
    T::Error: 'static,
{
    type Error = Error<T::Error>;

    #[inline]
    fn to_eflint(&self) -> Result<Vec<Phrase>, Self::Error> {
        let mut res: Vec<Phrase> = Vec::with_capacity(self.len());
        for (i, e) in self.iter().enumerate() {
            res.extend(e.to_eflint().map_err(|err| Error { i, err })?);
        }
        Ok(res)
    }
}
impl<const LEN: usize, T: EFlintable> EFlintable for [T; LEN]
where
    T::Error: 'static,
{
    type Error = Error<T::Error>;

    #[inline]
    fn to_eflint(&self) -> Result<Vec<Phrase>, Self::Error> { <[T] as EFlintable>::to_eflint(self) }
}
impl<T: EFlintable> EFlintable for Vec<T>
where
    T::Error: 'static,
{
    type Error = Error<T::Error>;

    #[inline]
    fn to_eflint(&self) -> Result<Vec<Phrase>, Self::Error> { <[T] as EFlintable>::to_eflint(self) }
}
