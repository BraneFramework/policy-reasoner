//  CONTEXT.rs
//    by Lut99
//
//  Created:
//    17 Oct 2024, 11:17:05
//  Last edited:
//    17 Oct 2024, 11:20:02
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines how the context of a reasoner looks like.
//

use std::borrow::Cow;

pub use serde::Serialize;


/***** LIBRARY *****/
/// Defines the context that is logged and deterministically determines reasoner behaviour.
///
/// In other words, same context == same result given a state.
pub trait Context: Serialize {
    /// Returns the (unique!) identifier of this reasoner.
    fn kind(&self) -> &str;
}

// Default impls for strings
impl Context for str {
    #[inline]
    fn kind(&self) -> &str { self }
}
impl<'a> Context for &'a str {
    #[inline]
    fn kind(&self) -> &str { self }
}
impl<'a> Context for &'a mut str {
    #[inline]
    fn kind(&self) -> &str { self }
}
impl<'a> Context for Cow<'a, str> {
    #[inline]
    fn kind(&self) -> &str { self }
}
impl Context for String {
    #[inline]
    fn kind(&self) -> &str { self.as_str() }
}
