//  STMT.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 14:24:22
//  Last edited:
//    10 Oct 2024, 14:32:46
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
    /// Logging a reasoner response.
    ReasonerResponse { reference: Cow<'a, str>, response: Cow<'a, ReasonerResponse<T>>, raw: Option<Cow<'a, str>> },
}
