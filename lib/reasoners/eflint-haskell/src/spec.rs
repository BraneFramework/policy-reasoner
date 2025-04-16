//  SPEC.rs
//    by Lut99
//
//  Created:
//    16 Apr 2025, 23:43:13
//  Last edited:
//    16 Apr 2025, 23:51:25
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines this crate's version of some interfaces necessary to make it
//!   work.
//

use std::borrow::Cow;
use std::fmt::{Display, Formatter, Result as FResult};


/***** FORMATTERS *****/
/// Formatter wrapping an [`EFlintable`] such that it implements [`Display`].
#[derive(Clone, Copy)]
pub struct EFlintableFormatter<'o, E: ?Sized> {
    /// Some object implementing [`EFlintable`].
    obj: &'o E,
}
impl<'o, E: ?Sized + EFlintable> Display for EFlintableFormatter<'o, E> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { self.obj.eflint_fmt(f) }
}





/***** LIBRARY *****/
/// A less-nice version of `eflint-json`'s `EFlintable`-trait.
///
/// It is less nice because we do not depend on some IR other than good ol' strings.
pub trait EFlintable {
    /// Writes an eFLINT (string) representation of this type to the given formatter.
    ///
    /// # Arguments
    /// - `f`: Some [`Formatter`] to write to.
    ///
    /// # Errors
    /// This function should error if it failed to write to the given formatter.
    fn eflint_fmt(&self, f: &mut Formatter<'_>) -> FResult;
}

// Practical impls
impl EFlintable for () {
    #[inline]
    fn eflint_fmt(&self, _f: &mut Formatter<'_>) -> FResult { Ok(()) }
}

// Pointer impls
impl<'a, T: ?Sized + EFlintable> EFlintable for &'a T {
    #[inline]
    fn eflint_fmt(&self, f: &mut Formatter<'_>) -> FResult { <T as EFlintable>::eflint_fmt(self, f) }
}
impl<'a, T: ?Sized + EFlintable> EFlintable for &'a mut T {
    #[inline]
    fn eflint_fmt(&self, f: &mut Formatter<'_>) -> FResult { <T as EFlintable>::eflint_fmt(self, f) }
}
impl<'a, T: ?Sized + Clone + EFlintable> EFlintable for Cow<'a, T> {
    #[inline]
    fn eflint_fmt(&self, f: &mut Formatter<'_>) -> FResult { <T as EFlintable>::eflint_fmt(self, f) }
}

// Container impls
impl<T: EFlintable> EFlintable for [T] {
    #[inline]
    fn eflint_fmt(&self, f: &mut Formatter<'_>) -> FResult {
        for elem in self {
            <T as EFlintable>::eflint_fmt(elem, f)?;
        }
        Ok(())
    }
}
impl<const LEN: usize, T: EFlintable> EFlintable for [T; LEN] {
    #[inline]
    fn eflint_fmt(&self, f: &mut Formatter<'_>) -> FResult {
        for elem in self {
            <T as EFlintable>::eflint_fmt(elem, f)?;
        }
        Ok(())
    }
}
impl<T: EFlintable> EFlintable for Vec<T> {
    #[inline]
    fn eflint_fmt(&self, f: &mut Formatter<'_>) -> FResult {
        for elem in self {
            <T as EFlintable>::eflint_fmt(elem, f)?;
        }
        Ok(())
    }
}



/// Extension upon an [`EFlintable`] to make it optionally nicer to work with.
pub trait EFlintableExt: EFlintable {
    /// Returns some formatter that implements [`Display`].
    ///
    /// # Returns
    /// An [`EFlintableFormatter`] that will use [`EFlintable::eflint_fmt()`] in order to write
    /// serialized eFLINT to some source.
    #[inline]
    fn eflint(&self) -> EFlintableFormatter<Self> { EFlintableFormatter { obj: self } }
}
impl<T: EFlintable> EFlintableExt for T {}
