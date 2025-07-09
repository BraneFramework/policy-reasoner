//  LIB.rs
//    by Lut99
//
//  Created:
//    09 Oct 2024, 15:50:24
//  Last edited:
//    11 Oct 2024, 15:45:44
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements eFLINT as a backend reasoner for the policy reasoner.
//

// Declare the modules
mod reasonerconn;
pub mod reasons;
pub mod spec;

// Use some of that in the crate namespace
pub use eflint_json as json;
pub use reasonerconn::*;
