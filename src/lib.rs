//  LIB.rs
//    by Lut99
//
//  Created:
//    08 Oct 2024, 16:13:30
//  Last edited:
//    17 Oct 2024, 12:02:50
//  Auto updated?
//    Yes
//
//  Description:
//!   A library for using several different reasoning backends to
//!   determine if a particular workflow is allowed by policy or not.
//

/// Contains the backend reasoners.
pub mod reasoners {
    #[cfg(feature = "eflint-json-reasoner")]
    pub use eflint_json_reasoner as eflint_json;
    #[cfg(feature = "no-op-reasoner")]
    pub use no_op_reasoner as no_op;
    #[cfg(feature = "posix-reasoner")]
    pub use posix_reasoner as posix;
}
/// Contains the backend loggers.
pub mod loggers {
    #[cfg(feature = "file-logger")]
    pub use file_logger as file;
    #[cfg(feature = "no-op-logger")]
    pub use no_op_logger as no_op;
}
/// Contains any state resolvers.
pub mod resolvers {
    #[cfg(feature = "file-resolver")]
    pub use file_resolver as file;
}
#[cfg(feature = "eflint-to-json")]
pub use eflint_to_json;
pub use spec;
#[cfg(feature = "workflow")]
pub use workflow;
