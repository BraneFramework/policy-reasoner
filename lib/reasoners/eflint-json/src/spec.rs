//  SPEC.rs
//    by Lut99
//
//  Created:
//    09 Oct 2024, 16:06:18
//  Last edited:
//    10 Oct 2024, 14:06:45
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines some general interface for this crate.
//

use std::convert::Infallible;
use std::error::Error;

use eflint_json::spec::Phrase;


/***** LIBRARY *****/
/// Defines something that can be turned into eFLINT phrases.
pub trait EFlintable {
    /// The error type returned when converting to eFLINT.
    type Error: Error;


    /// Converts this state to eFLINT phrases.
    ///
    /// # Returns
    /// A list of [`Phrase`]s that represent the eFLINT to send to the reasoner.
    ///
    /// # Errors
    /// This function can fail if `self` is not in a right state to be serialized to eFLINT.
    fn to_eflint(&self) -> Result<Vec<Phrase>, Self::Error>;
}

// Default impls
impl EFlintable for () {
    type Error = Infallible;

    #[inline]
    fn to_eflint(&self) -> Result<Vec<Phrase>, Self::Error> { Ok(Vec::new()) }
}
