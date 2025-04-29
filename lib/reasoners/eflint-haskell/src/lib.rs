//  LIB.rs
//    by Lut99
//
//  Created:
//    16 Apr 2025, 23:09:00
//  Last edited:
//    29 Apr 2025, 15:38:06
//  Auto updated?
//    Yes
//
//  Description:
//!   Wraps the infamous Haskell interpreter to supply an eFLINT DSL
//!   backend.
//

// Define the submodules
pub mod reasonerconn;
pub mod reasons;
pub mod spec;
pub mod trace;

// Use some of that
pub use reasonerconn::*;
