//  STMT.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 14:24:22
//  Last edited:
//    17 Oct 2024, 13:15:51
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the internal representation of a log statement.
//

use std::borrow::Cow;

use enum_debug::EnumDebug;
use serde::{Deserialize, Serialize};
use serde_json::Value;


/***** LIBRARY *****/
/// Defines the internal representation of a log statement.
#[derive(Clone, Debug, Deserialize, EnumDebug, Serialize)]
pub enum LogStatement<'a> {
    /// Logging a reasoner context.
    Context { context: Value },
    /// Logging a question to a reasoner.
    ReasonerConsult { reference: Cow<'a, str>, state: Value, question: Value },
    /// Logging a reasoner response.
    ReasonerResponse { reference: Cow<'a, str>, response: Value, raw: Option<Cow<'a, str>> },
}
