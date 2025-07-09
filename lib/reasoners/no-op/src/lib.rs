//  LIB.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 16:19:50
//  Last edited:
//    11 Oct 2024, 16:23:50
//  Auto updated?
//    Yes
//
//  Description:
//!   A minimal policy reasoner implementation that can be used as a base
//!   for new policy reasoners.
//!   
//!   This no-operation reasoner is meant to be an example, and can be used as
//!   a base to build new reasoners on top of. Furthermore it can be used for
//!   testing. The reasoner approves all workflow validation requests by
//!   default (it does not perform any permission checks, and thus never
//!   rejects a request).
//

// Declare the modules
mod reasonerconn;

// Bring it into this namespace
pub use reasonerconn::*;
