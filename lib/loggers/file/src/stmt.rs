//  STMT.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 14:24:22
//  Last edited:
//    17 Oct 2024, 09:49:04
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the internal representation of a log statement.
//

use std::borrow::Cow;

use enum_debug::EnumDebug;
use serde::{Deserialize, Serialize};
use spec::reasonerconn::ReasonerResponse;


/***** LIBRARY *****/
/// Defines the internal representation of a log statement.
#[derive(Clone, Debug, Deserialize, EnumDebug, Serialize)]
pub enum LogStatement<'a, T: Clone> {
    /// Logging a reasoner context.
    Context { context: Cow<'a, str> },
    /// Logging a reasoner response.
    ReasonerResponse { reference: Cow<'a, str>, response: Cow<'a, ReasonerResponse<T>>, raw: Option<Cow<'a, str>> },
}
