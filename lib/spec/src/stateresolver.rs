//  STATERESOLVER.rs
//    by Lut99
//
//  Created:
//    10 Oct 2024, 14:57:24
//  Last edited:
//    05 Nov 2024, 10:55:01
//  Auto updated?
//    Yes
//
//  Description:
//!   Provides an interface to state resolvers.
//

use std::error::Error;
use std::future::Future;

use paste::paste;

use crate::auditlogger::{AuditLogger, SessionedAuditLogger};


/***** HELPER MACRO *****/
/// Implements the [`StateResolver`] for tuples of the given sizes.
macro_rules! tuple_impls {
    // Public interface
    ($fi:tt $(, $i:tt)*) => {
        // First, let's reverse the tuple
        tuple_impls!(pair ($fi, $fi): $($i),* :);
    };

    // Pair up the items
    (pair ($fi:tt, $pi:tt): : $(($pri:tt, $ri:tt)),*) => {
        tuple_impls!(reverse ($fi): $(($pri, $ri)),*:);
    };
    (pair ($fi:tt, $pi:tt): $i1:tt $(, $i:tt)*: $(($pri:tt, $ri:tt)),*) => {
        tuple_impls!(pair ($fi, $i1): $($i),*: $(($pri, $ri),)* ($pi, $i1));
    };

    // Reverse the items
    (reverse ($fi:tt): : $(($pri:tt, $ri:tt)),*) => {};
    (reverse ($fi:tt): ($pi1:tt, $i1:tt) $(, ($pi:tt, $i:tt))* : $(($pri:tt, $ri:tt)),*) => {
        tuple_impl!($fi, $i1, $(($pri, $ri),)* ($pi1, $i1));
        tuple_impls!(reverse ($fi): $(($pi, $i)),* : $(($pri, $ri),)* ($pi1, $i1));
    };
}

/// Implements the [`StateResolver`] for a tuple of a specific length.
macro_rules! tuple_impl {
    ($fi:tt, $li:tt, $(($pi:tt, $i:tt)),*) => {
        paste! {
            impl<E, [<T $fi>] $(, [<T $i>])*> StateResolver for ([<T $fi>] $(, [<T $i>])*)
            where
                E: std::error::Error,
                [<T $fi>]: Sync + StateResolver<Error = E>,
                [<T $fi>]::State: Send,
                $(
                    [<T $i>]: Sync + StateResolver<State = [<T $pi>]::Resolved, Error = E>,
                    [<T $i>]::State: Send,
                )*
            {
                type State = [<T $fi>]::State;
                type Resolved = [<T $li>]::Resolved;
                type Error = E;

                async fn resolve<'a, L>(&'a self, state: Self::State, logger: &'a SessionedAuditLogger<L>) -> Result<Self::Resolved, Self::Error>
                where
                    L: Sync + AuditLogger,
                {
                    let resolved: [<T $fi>]::Resolved = self.$fi.resolve(state, logger).await?;
                    $(let resolved: [<T $i>]::Resolved = self.$i.resolve(resolved, logger).await?;)*
                    Ok(resolved)
                }
            }
        }
    };
}





/***** LIBRARY *****/
/// Defines an interface that sits before a reasoner to collect its state.
pub trait StateResolver {
    /// Defines the state that is being resolved.
    type State;
    /// Defines the state after resolution; may be the same.
    type Resolved;
    /// Defines the errors occurring during resolution.
    type Error: Error;


    /// Resolves the given state.
    ///
    /// # Arguments
    /// - `state`: The [`StateResolver::State`] that we already have and want to (further) resolve.
    /// - `logger`: A [`SessionedAuditLogger`] wrapping some [`AuditLogger`] that is used to write to the audit trail as the question's being asked.
    ///
    /// # Returns
    /// A [`StateResolver::Resolved`] that represents the (more) resolved version of `state`.
    ///
    /// # Errors
    /// This function may error if it failed to do its resolution.
    fn resolve<'a, L>(&'a self, state: Self::State, logger: &'a SessionedAuditLogger<L>) -> impl Future<Output = Result<Self::Resolved, Self::Error>>
    where
        L: Sync + AuditLogger;
}

// Default impls
tuple_impls!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
