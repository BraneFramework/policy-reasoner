//  TRACE.rs
//    by Lut99
//
//  Created:
//    17 Apr 2025, 00:06:39
//  Last edited:
//    06 May 2025, 12:55:32
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements a little IR for parsed traces produced by the eFLINT
//!   reasoner.
//

use std::convert::Infallible;
use std::error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;


/***** CONSTANTS *****/
const DISABLED_ACTION: &str = "disabled action:";
const EXEC_TRANS: &str = "executed transition:";
const NEW_INVARIANT: &str = "New invariant";
const NEW_TYPE: &str = "New type";
const TRANS_ENABLED: &str = "(ENABLED)";
const TRANS_DISABLED: &str = "(DISABLED)";
const QUERY_SUCCESS: &str = "query successful";
const QUERY_FAILED: &str = "query failed";
const VIOLATIONS: &str = "violations:";
const VIOLATED_DUTY: &str = "violated duty!:";
const VIOLATED_INVARIANT: &str = "violated invariant!:";





/***** ERRORS *****/
/// Defines fatal parsing errors for parsing traces [from strings](FromStrHead::from_str_head()).
#[derive(Debug, Error, Eq, PartialEq)]
pub enum Error {
    #[error("Expected a comma at {s:?}")]
    ExpectedComma { s: String },
    #[error("Expected \"`\" to follow \"|\" while parsing transition trees at {s:?}")]
    ExpectedHookAfterBar { s: String },
    #[error("Expected instance to follow \"disabled action\" at {s:?}")]
    ExpectedInstanceAfterDisabledAction { s: String },
    #[error("Expected instance to follow \"violated duty!\" at {s:?}")]
    ExpectedInstanceAfterViolatedDuty { s: String },
    #[error("Expected \"-\" to follow \"`\" while parsing transition trees at {s:?}")]
    ExpectedPipeAfterHook { s: String },
    #[error("Expected type name to follow \"New invariant\" at {s:?}")]
    ExpectedTypeNameAfterNewInvariant { s: String },
    #[error("Expected type name to follow \"New type\" at {s:?}")]
    ExpectedTypeNameAfterNewType { s: String },
    #[error("Expected type name to follow \"violated invariant!\" at {s:?}")]
    ExpectedTypeNameAfterViolatedInvariant { s: String },
    #[error("Expected instance after magic 'executed transition' keyword at {s:?}")]
    MissingInstanceAfterExecuted { s: String },
    #[error("Out-of-range integer at {s:?}")]
    OutOfRangeInt { s: String },
    #[error("Failed to parse instance following postulation op {op} at {s:?}")]
    PostulationOpWithoutInstance { op: PostulationOp, s: String },
    #[error("Unparsable input at {s:?}")]
    UnparsableInput { s: String },
    #[error("Expected closing delimiter {delim:?} for opening delimiter starting at {s:?}")]
    UnterminatedDelim { delim: char, s: String },
    #[error("Expected closing parenthesis at {s:?}")]
    UnterminatedParen { s: String },
    #[error("Unterminated string at {s:?}")]
    UnterminatedString { s: String },
}





/***** INTERFACES *****/
/// Generalizes parsing for all of the trace nodes.
pub trait FromStrHead {
    /// The type of error emitted when parsing fails.
    type Error: error::Error;

    /// Parses an instance of this type from the head of the given string.
    ///
    /// Any unparsed string is returned.
    ///
    /// # Arguments
    /// - `s`: The input string to parse from.
    ///
    /// # Returns
    /// A tuple of the remaining string, if any, and the parsed `self`.
    ///
    /// If this input was not recognized, then [`None`] is returned instead.
    ///
    /// # Errors
    /// This function errors if the head of the input was recognized as an instance but invalid.
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error>
    where
        Self: Sized;
}





/***** LIBRARY *****/
/// Defines a trace as a whole.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Trace {
    /// The deltas emitted by eFLINT.
    pub deltas: Vec<Delta>,
}
impl Display for Trace {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> FResult {
        for delta in &self.deltas {
            delta.fmt(f)?;
            writeln!(f)?;
        }
        Ok(())
    }
}
impl FromStr for Trace {
    type Err = Error;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Ensure all of the input is consumed this time
        // SAFETY: Note that `Self::from_str_head()` actually never yields `None`
        let (rem, this): (&str, Self) = Self::from_str_head(s)?.unwrap();
        if rem.trim().is_empty() { Ok(this) } else { Err(Error::UnparsableInput { s: rem.into() }) }
    }
}
impl FromStrHead for Trace {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        let mut deltas: Vec<Delta> = Vec::new();
        let mut rem = s.trim_start();
        while let Some((newrem, newdeltas)) = Vec::<Delta>::from_str_head(rem)? {
            deltas.extend(newdeltas);
            rem = newrem.trim_start();
        }
        Ok(Some((rem, Self { deltas })))
    }
}



/// Defines a delta, which is like the toplevel instance of the trace.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Delta {
    /// A type is marked as an invariant.
    NewInvariant(NewInvariant),
    /// A type definition.
    NewType(NewType),
    /// It's a postulation - i.e., a database update.
    Postulation(Postulation),
    /// It's a query - i.e., the answer to a question.
    ///
    /// Note we wouldn't know the question itself, actually.
    Query(Query),
    /// It's a trigger - i.e., a transition.
    Trigger(Trigger),
    /// It's a violation - i.e., an illegal state.
    Violation(Violation),
}
impl Display for Delta {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            Self::NewInvariant(ni) => ni.fmt(f),
            Self::NewType(nt) => nt.fmt(f),
            Self::Postulation(p) => p.fmt(f),
            Self::Query(q) => q.fmt(f),
            Self::Trigger(t) => t.fmt(f),
            Self::Violation(v) => v.fmt(f),
        }
    }
}
impl FromStrHead for Vec<Delta> {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        if let Some((rem, nin)) = NewInvariant::from_str_head(s)? {
            return Ok(Some((rem, vec![Delta::NewInvariant(nin)])));
        }
        if let Some((rem, nty)) = NewType::from_str_head(s)? {
            return Ok(Some((rem, vec![Delta::NewType(nty)])));
        }
        if let Some((rem, pos)) = Postulation::from_str_head(s)? {
            return Ok(Some((rem, vec![Delta::Postulation(pos)])));
        }
        if let Some((rem, quer)) = Query::from_str_head(s).unwrap() {
            return Ok(Some((rem, vec![Delta::Query(quer)])));
        }
        if let Some((rem, trigs)) = Vec::<Trigger>::from_str_head(s)? {
            return Ok(Some((rem, trigs.into_iter().map(Delta::Trigger).collect())));
        }
        if let Some((rem, viols)) = Vec::<Violation>::from_str_head(s)? {
            return Ok(Some((rem, viols.into_iter().map(Delta::Violation).collect())));
        }
        Ok(None)
    }
}

/// Defines an invariant definition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NewInvariant {
    /// The name of the newly defined invariant.
    pub name: String,
}
impl Display for NewInvariant {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "Marked \"{}\" as invariant", self.name) }
}
impl FromStrHead for NewInvariant {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        // Parse the magic first
        if !s.starts_with(NEW_INVARIANT) {
            return Ok(None);
        }
        let rem = s[NEW_INVARIANT.len()..].trim_start();

        // Then parse the type name
        match TypeName::from_str_head(rem)? {
            Some((rem, TypeName(name))) => Ok(Some((rem, Self { name }))),
            None => Err(Error::ExpectedTypeNameAfterNewInvariant { s: rem.into() }),
        }
    }
}

/// Defines a type definition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NewType {
    /// The name of the newly defined type.
    pub name: String,
}
impl Display for NewType {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "New type \"{}\"", self.name) }
}
impl FromStrHead for NewType {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        // Parse the magic first
        if !s.starts_with(NEW_TYPE) {
            return Ok(None);
        }
        let rem = s[NEW_TYPE.len()..].trim_start();

        // Then parse the type name
        match TypeName::from_str_head(rem)? {
            Some((rem, TypeName(name))) => Ok(Some((rem, Self { name }))),
            None => Err(Error::ExpectedTypeNameAfterNewType { s: rem.into() }),
        }
    }
}

/// Defines the answer to a query.
///
/// Note this is just the answer. The rest we wouldn't know.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Query {
    /// The answer is yes
    Succes,
    /// The answer is no
    Fail,
}
impl Query {
    /// Returns true if this query was a success.
    ///
    /// # Returns
    /// True if this is a [`Query::Succes`], or false otherwise.
    #[inline]
    pub const fn is_succes(&self) -> bool { matches!(self, Self::Succes) }
}
impl Display for Query {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { if self.is_succes() { write!(f, "Query succes") } else { write!(f, "Query failed") } }
}
impl FromStrHead for Query {
    type Error = Infallible;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        if let Some(rem) = s.strip_prefix(QUERY_SUCCESS) {
            return Ok(Some((rem, Self::Succes)));
        }
        if let Some(rem) = s.strip_prefix(QUERY_FAILED) {
            return Ok(Some((rem, Self::Fail)));
        }
        Ok(None)
    }
}

/// Defines a postulation delta.
///
/// This means that a new fact has been created, terminated or obfuscated.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Postulation {
    /// The operation applied.
    pub op:   PostulationOp,
    /// The instance that was postulated.
    pub inst: Instance,
}
impl Display for Postulation {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self.op {
            PostulationOp::Create => write!(f, "Created {}", self.inst),
            PostulationOp::Terminate => write!(f, "Terminated {}", self.inst),
            PostulationOp::Obfuscate => write!(f, "Obfuscated {}", self.inst),
        }
    }
}
impl FromStrHead for Postulation {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        // Parse the postulation op first
        let (rem, op): (&str, PostulationOp) = match PostulationOp::from_str_head(s).unwrap() {
            Some(res) => res,
            None => return Ok(None),
        };

        // Then any whitespace
        let rem = rem.trim_start();

        // Finally the instance
        match Instance::from_str_head(rem)? {
            Some((rem, inst)) => Ok(Some((rem, Self { op, inst }))),
            None => Err(Error::PostulationOpWithoutInstance { op, s: s.into() }),
        }
    }
}

/// Defines the possible kinds of postulation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PostulationOp {
    /// Transitioning a fact to be true (`+`).
    Create,
    /// Transitioning a fact to be false (`-`).
    Terminate,
    /// Transitioning to undo a postulation (`~`).
    Obfuscate,
}
impl Display for PostulationOp {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            Self::Create => write!(f, "+"),
            Self::Terminate => write!(f, "-"),
            Self::Obfuscate => write!(f, "~"),
        }
    }
}
impl FromStrHead for PostulationOp {
    type Error = Infallible;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        match s.chars().next() {
            Some('+') => Ok(Some((&s[1..], Self::Create))),
            Some('-') => Ok(Some((&s[1..], Self::Terminate))),
            Some('~') => Ok(Some((&s[1..], Self::Obfuscate))),
            _ => Ok(None),
        }
    }
}

/// Defines a triggered instance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Trigger {
    /// The triggered instance.
    pub inst:    Instance,
    /// Whether it was enabled or not.
    ///
    /// Not given for events.
    pub enabled: Option<bool>,
}
impl Display for Trigger {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        write!(f, "Triggered {}{}", self.inst, match self.enabled {
            Some(true) => " (ENABLED)",
            Some(false) => " (DISABLED)",
            None => "",
        })
    }
}
impl FromStrHead for Vec<Trigger> {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        // Parse the magic prefix first
        if !s.starts_with(EXEC_TRANS) {
            return Ok(None);
        }
        let rem = s[EXEC_TRANS.len()..].trim_start();

        // Now parse the triggered instance
        let (rem, inst): (&str, Instance) = match Instance::from_str_head(rem)? {
            Some(res) => res,
            None => return Err(Error::MissingInstanceAfterExecuted { s: rem.into() }),
        };

        // Parse the optional 'ENABLED|DISABLED' bizz
        let rem = rem.trim_start();
        let (mut rem, enabled): (&str, Option<bool>) = if let Some(rem) = rem.strip_prefix(TRANS_ENABLED) {
            (rem.trim_start(), Some(true))
        } else if let Some(rem) = rem.strip_prefix(TRANS_DISABLED) {
            (rem.trim_start(), Some(false))
        } else {
            (rem, None)
        };

        // Now we will parse an optional tree of triggered instances, if `Syncs with` is used.
        let mut result: Vec<Trigger> = vec![Trigger { inst, enabled }];
        while rem.starts_with('|') {
            // Pop the rest of the tree symbol
            let newrem = rem[1..].trim_start();
            if !newrem.starts_with('`') {
                return Err(Error::ExpectedHookAfterBar { s: newrem.into() });
            }
            let newrem = newrem[1..].trim_start();
            if !newrem.starts_with('-') {
                return Err(Error::ExpectedPipeAfterHook { s: newrem.into() });
            }
            let newrem = newrem[1..].trim_start();

            // Now parse a new instance / enabled pair
            let (newrem, inst): (&str, Instance) = match Instance::from_str_head(newrem)? {
                Some(res) => res,
                None => return Err(Error::MissingInstanceAfterExecuted { s: newrem.into() }),
            };

            // Parse the optional 'ENABLED|DISABLED' bizz
            let newrem = newrem.trim_start();
            let (newrem, enabled): (&str, Option<bool>) = if let Some(rem) = newrem.strip_prefix(TRANS_ENABLED) {
                (rem.trim_start(), Some(true))
            } else if let Some(rem) = newrem.strip_prefix(TRANS_DISABLED) {
                (rem.trim_start(), Some(false))
            } else {
                (newrem, None)
            };

            // Try again
            result.push(Trigger { inst, enabled });
            rem = newrem;
        }

        // Done!
        Ok(Some((rem, result)))
    }
}

/// Defines any violation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Violation {
    /// An action has been violated.
    Act(ActViolation),
    /// A duty has been violated.
    Duty(DutyViolation),
    /// An invariant has been violated.
    Invariant(InvariantViolation),
}
impl Display for Violation {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            Self::Act(a) => a.fmt(f),
            Self::Duty(d) => d.fmt(f),
            Self::Invariant(i) => i.fmt(f),
        }
    }
}
impl FromStrHead for Vec<Violation> {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        // Parse the initial magic
        if !s.starts_with(VIOLATIONS) {
            return Ok(None);
        }
        let mut rem = s[VIOLATIONS.len()..].trim_start();

        // Now keep popping violations
        let mut res: Vec<Violation> = Vec::new();
        loop {
            if let Some((newrem, act)) = ActViolation::from_str_head(rem)? {
                res.push(Violation::Act(act));
                rem = newrem.trim_start();
            } else if let Some((newrem, duty)) = DutyViolation::from_str_head(rem)? {
                res.push(Violation::Duty(duty));
                rem = newrem.trim_start();
            } else if let Some((newrem, inv)) = InvariantViolation::from_str_head(rem)? {
                res.push(Violation::Invariant(inv));
                rem = newrem.trim_start();
            } else {
                return Ok(Some((rem, res)));
            }
        }
    }
}

/// Defines the violation of an act.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActViolation {
    /// The violated instance.
    pub inst: Composite,
}
impl Display for ActViolation {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "Violated action {}", self.inst) }
}
impl FromStrHead for ActViolation {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        // Parse the prompt first
        if !s.starts_with(DISABLED_ACTION) {
            return Ok(None);
        }
        let rem = s[DISABLED_ACTION.len()..].trim_start();

        // Then parse the instance that was violated
        match Composite::from_str_head(rem)? {
            Some((rem, inst)) => Ok(Some((rem, ActViolation { inst }))),
            None => Err(Error::ExpectedInstanceAfterDisabledAction { s: rem.into() }),
        }
    }
}

/// Defines the violation of a duty.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DutyViolation {
    /// The violated instance.
    pub inst: Composite,
}
impl Display for DutyViolation {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "Violated duty {}", self.inst) }
}
impl FromStrHead for DutyViolation {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        // Parse the prompt first
        if !s.starts_with(VIOLATED_DUTY) {
            return Ok(None);
        }
        let rem = s[VIOLATED_DUTY.len()..].trim_start();

        // Then parse the instance that was violated
        match Composite::from_str_head(rem)? {
            Some((rem, inst)) => Ok(Some((rem, DutyViolation { inst }))),
            None => Err(Error::ExpectedInstanceAfterViolatedDuty { s: rem.into() }),
        }
    }
}

/// Defines the violation of an invariant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InvariantViolation {
    /// The violated invariant.
    pub name: String,
}
impl Display for InvariantViolation {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "Violated invariant {}", self.name) }
}
impl FromStrHead for InvariantViolation {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        // Parse the prompt first
        if !s.starts_with(VIOLATED_INVARIANT) {
            return Ok(None);
        }
        let rem = s[VIOLATED_INVARIANT.len()..].trim_start();

        // Then parse the invariant that was violated
        match TypeName::from_str_head(rem)? {
            Some((rem, TypeName(name))) => Ok(Some((rem, InvariantViolation { name }))),
            None => Err(Error::ExpectedTypeNameAfterViolatedInvariant { s: rem.into() }),
        }
    }
}



/// Defines an instance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Instance {
    /// A naked string literal.
    StringLit(StringLit),
    /// A naked int literal.
    IntLit(IntLit),
    /// A composite type.
    Composite(Composite),
}
impl Display for Instance {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            Self::StringLit(sl) => sl.fmt(f),
            Self::IntLit(il) => il.fmt(f),
            Self::Composite(c) => c.fmt(f),
        }
    }
}
impl FromStrHead for Instance {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        if let Some((rem, lit)) = StringLit::from_str_head(s)? {
            return Ok(Some((rem, Instance::StringLit(lit))));
        }
        if let Some((rem, lit)) = IntLit::from_str_head(s)? {
            return Ok(Some((rem, Instance::IntLit(lit))));
        }
        if let Some((rem, comp)) = Composite::from_str_head(s)? {
            return Ok(Some((rem, Instance::Composite(comp))));
        }
        Ok(None)
    }
}

/// Defines a string literal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StringLit(pub String);
impl Display for StringLit {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "{:?}", self.0) }
}
impl FromStrHead for StringLit {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        // Feel like I'm always writing these. Here we go.

        // Match the opening quote
        let mut chars = s.char_indices();
        if !matches!(chars.next(), Some((0, '"'))) {
            return Ok(None);
        }

        // Parse the remainder in a lil' state machine
        enum State {
            Body,
            Escaped,
        }
        let mut state = State::Body;
        let mut value: String = String::new();
        for (i, c) in chars {
            match state {
                State::Body if c == '"' => return Ok(Some((&s[i + 1..], StringLit(value)))),
                State::Body if c == '\\' => state = State::Escaped,
                State::Body => value.push(c),

                State::Escaped if c == 'n' => {
                    value.push('\n');
                    state = State::Body;
                },
                State::Escaped if c == 't' => {
                    value.push('\t');
                    state = State::Body;
                },
                State::Escaped if c == 'r' => {
                    value.push('\r');
                    state = State::Body;
                },
                State::Escaped => {
                    value.push(c);
                    state = State::Body;
                },
            }
        }

        // We ran out of input
        Err(Error::UnterminatedString { s: s.into() })
    }
}

/// Defines an integer literal.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IntLit(pub i64);
impl Display for IntLit {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "{}", self.0) }
}
impl FromStrHead for IntLit {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        // Parse negatives
        let mut chars = s.char_indices();
        let mut modifier: i64 = 1;
        let mut next = chars.next();
        while let Some((_, '-')) = next {
            modifier *= -1;
            next = chars.next();
        }

        // Parse the integer body
        let mut value: i64 = 0;
        let mut seen_one: bool = false;
        for (i, c) in next.into_iter().chain(chars) {
            if c.is_ascii_digit() {
                // Multiply the existing value by 10 to "move them left"
                if modifier > 0 && value > i64::MAX / 10 || modifier < 0 && value < i64::MIN / 10 {
                    return Err(Error::OutOfRangeInt { s: s.into() });
                }
                value *= 10;

                // Add the digit
                let digit: i64 = modifier * ((c as i64) - ('0' as i64));
                if modifier > 0 && value > i64::MAX - digit || modifier < 0 && value < i64::MIN - digit {
                    return Err(Error::OutOfRangeInt { s: s.into() });
                }
                value += digit;

                // Remember we've seen this one
                seen_one = true;
            } else if seen_one {
                return Ok(Some((&s[i..], IntLit(value))));
            } else {
                return Ok(None);
            }
        }

        // If we got here, then there were 0 digits
        if seen_one { Ok(Some((&s[s.len()..], IntLit(value)))) } else { Ok(None) }
    }
}

/// Defines a composite type.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Composite {
    /// The name of the type.
    pub name: String,
    /// The arguments of the type.
    pub args: Vec<Instance>,
}
impl Display for Composite {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        write!(f, "{}({})", self.name, self.args.iter().map(Instance::to_string).collect::<Vec<String>>().join(", "))
    }
}
impl FromStrHead for Composite {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        // Parse an identifier type first
        let (rem, name): (&str, String) = match TypeName::from_str_head(s)? {
            Some((rem, TypeName(name))) => (rem, name),
            None => return Ok(None),
        };

        // Parse whitespace, then an opening parenthesis
        let rem = rem.trim_start();
        if !matches!(rem.chars().next(), Some('(')) {
            return Ok(None);
        }
        let mut rem = rem[1..].trim_start();

        // Parse instances delimited by commas
        let mut args: Vec<Instance> = Vec::new();
        while let Some((newrem, inst)) = Instance::from_str_head(rem)? {
            // Accept the instance
            args.push(inst);
            rem = newrem;

            // Parse a comma
            rem = rem.trim_start();
            let next = rem.chars().next();
            if matches!(next, Some(')')) {
                return Ok(Some((&rem[1..], Self { name, args })));
            } else if !matches!(next, Some(',')) {
                return Err(Error::ExpectedComma { s: newrem.into() });
            }
            rem = rem[1..].trim_start();
        }

        // If we got here, not enough time to find closing parenthesis
        if matches!(rem.chars().next(), Some(')')) {
            Ok(Some((&rem[1..], Self { name, args })))
        } else {
            Err(Error::UnterminatedParen { s: rem.into() })
        }
    }
}



/// Parses an eFLINT type name.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TypeName(pub String);
impl FromStrHead for TypeName {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        enum Kind {
            Plain,
            Brackets,
            Chevrons,
            Uninit,
        }

        let mut kind = Kind::Uninit;
        let mut depth: usize = 1;
        for (i, c) in s.char_indices() {
            if i == 0 {
                if c.is_ascii_lowercase() || c == '_' || c == '-' {
                    kind = Kind::Plain;
                } else if c == '[' {
                    kind = Kind::Brackets;
                } else if c == '<' {
                    kind = Kind::Chevrons;
                } else {
                    return Ok(None);
                }
            } else if matches!(kind, Kind::Plain) {
                // Stop when we find a non-valid character
                if !c.is_ascii_lowercase() && !c.is_ascii_uppercase() && c != '-' && c != '_' {
                    return Ok(Some((&s[i..], Self(s[..i].into()))));
                }
            } else if matches!(kind, Kind::Brackets) {
                // Stop when we find a closing bracket at depth 1
                if c == ']' && depth == 1 {
                    return Ok(Some((&s[i + 1..], Self(s[..i + 1].into()))));
                } else if c == ']' {
                    depth -= 1;
                } else if c == '[' {
                    depth += 1;
                }
            } else if matches!(kind, Kind::Chevrons) {
                // Stop when we find a closing chevron at depth 1
                if c == '>' && depth == 1 {
                    return Ok(Some((&s[i + 1..], Self(s[..i + 1].into()))));
                } else if c == '>' {
                    depth -= 1;
                } else if c == '<' {
                    depth += 1;
                }
            } else {
                unreachable!();
            }
        }

        // No input
        if matches!(kind, Kind::Plain) {
            Ok(Some((&s[s.len()..], Self(s.into()))))
        } else if matches!(kind, Kind::Uninit) {
            Ok(None)
        } else {
            Err(Error::UnterminatedDelim { delim: (if matches!(kind, Kind::Brackets) { ']' } else { '>' }), s: s.into() })
        }
    }
}





/***** TESTS *****/
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_newtype() {
        assert_eq!(NewType::from_str_head("New type foo"), Ok(Some(("", NewType { name: "foo".into() }))));
        assert_eq!(NewType::from_str_head("New type a"), Ok(Some(("", NewType { name: "a".into() }))));
        assert_eq!(NewType::from_str_head("New type a a"), Ok(Some((" a", NewType { name: "a".into() }))));
        assert_eq!(NewType::from_str_head("New type"), Err(Error::ExpectedTypeNameAfterNewType { s: "".into() }));
        assert_eq!(NewType::from_str_head("ew type foo"), Ok(None));
        assert_eq!(NewType::from_str_head(""), Ok(None));
    }

    #[test]
    fn test_parse_query() {
        assert_eq!(Query::from_str_head("query successful"), Ok(Some(("", Query::Succes))));
        assert_eq!(Query::from_str_head("query failed"), Ok(Some(("", Query::Fail))));
        assert_eq!(Query::from_str_head("query successfulAND MORE"), Ok(Some(("AND MORE", Query::Succes))));
        assert_eq!(Query::from_str_head("query successfu"), Ok(None));
        assert_eq!(Query::from_str_head("aquery failed"), Ok(None));
        assert_eq!(Query::from_str_head(""), Ok(None));
    }

    #[test]
    fn test_parse_postulation() {
        assert_eq!(Postulation::from_str_head("+42"), Ok(Some(("", Postulation { op: PostulationOp::Create, inst: Instance::IntLit(IntLit(42)) }))));
        assert_eq!(
            Postulation::from_str_head("+-42"),
            Ok(Some(("", Postulation { op: PostulationOp::Create, inst: Instance::IntLit(IntLit(-42)) })))
        );
        assert_eq!(
            Postulation::from_str_head("---42"),
            Ok(Some(("", Postulation { op: PostulationOp::Terminate, inst: Instance::IntLit(IntLit(42)) })))
        );
        assert_eq!(
            Postulation::from_str_head("+foo(\"Amy\")"),
            Ok(Some(("", Postulation {
                op:   PostulationOp::Create,
                inst: Instance::Composite(Composite { name: "foo".into(), args: vec![Instance::StringLit(StringLit("Amy".into()))] }),
            })))
        );
        assert_eq!(
            Postulation::from_str_head("-bar()"),
            Ok(Some(("", Postulation { op: PostulationOp::Terminate, inst: Instance::Composite(Composite { name: "bar".into(), args: vec![] }) })))
        );
        assert_eq!(
            Postulation::from_str_head("~baz(foo(\"Bob\"), 42)"),
            Ok(Some(("", Postulation {
                op:   PostulationOp::Obfuscate,
                inst: Instance::Composite(Composite {
                    name: "baz".into(),
                    args: vec![
                        Instance::Composite(Composite { name: "foo".into(), args: vec![Instance::StringLit(StringLit("Bob".into()))] }),
                        Instance::IntLit(IntLit(42))
                    ],
                }),
            })))
        );
        assert_eq!(Postulation::from_str_head("foo(\"Amy\")"), Ok(None));
        assert_eq!(Postulation::from_str_head(""), Ok(None));
    }

    #[test]
    fn test_parse_postulation_op() {
        assert_eq!(PostulationOp::from_str_head("+"), Ok(Some(("", PostulationOp::Create))));
        assert_eq!(PostulationOp::from_str_head("-"), Ok(Some(("", PostulationOp::Terminate))));
        assert_eq!(PostulationOp::from_str_head("~"), Ok(Some(("", PostulationOp::Obfuscate))));
        assert_eq!(PostulationOp::from_str_head("+a"), Ok(Some(("a", PostulationOp::Create))));
        assert_eq!(PostulationOp::from_str_head("a"), Ok(None));
        assert_eq!(PostulationOp::from_str_head(""), Ok(None));
    }

    #[test]
    fn test_parse_trigger() {
        assert_eq!(
            Vec::<Trigger>::from_str_head("executed transition:\ngo(string(\"y\"))"),
            Ok(Some(("", vec![Trigger {
                inst:    Instance::Composite(Composite {
                    name: "go".into(),
                    args: vec![Instance::Composite(Composite { name: "string".into(), args: vec![Instance::StringLit(StringLit("y".into()))] })],
                }),
                enabled: None,
            }])))
        );
        assert_eq!(
            Vec::<Trigger>::from_str_head("executed transition: \ngo(string(\"y\")) (ENABLED)"),
            Ok(Some(("", vec![Trigger {
                inst:    Instance::Composite(Composite {
                    name: "go".into(),
                    args: vec![Instance::Composite(Composite { name: "string".into(), args: vec![Instance::StringLit(StringLit("y".into()))] })],
                }),
                enabled: Some(true),
            }])))
        );
        assert_eq!(
            Vec::<Trigger>::from_str_head("executed transition: go(string(\"y\")) (DISABLED)"),
            Ok(Some(("", vec![Trigger {
                inst:    Instance::Composite(Composite {
                    name: "go".into(),
                    args: vec![Instance::Composite(Composite { name: "string".into(), args: vec![Instance::StringLit(StringLit("y".into()))] })],
                }),
                enabled: Some(false),
            }])))
        );
        assert_eq!(
            Vec::<Trigger>::from_str_head(
                r#"executed transition: 
                go(string("y")) (DISABLED)
                |
                `- go(string("x")) (DISABLED)
                "#
            ),
            Ok(Some(("", vec![
                Trigger {
                    inst:    Instance::Composite(Composite {
                        name: "go".into(),
                        args: vec![Instance::Composite(Composite { name: "string".into(), args: vec![Instance::StringLit(StringLit("y".into()))] })],
                    }),
                    enabled: Some(false),
                },
                Trigger {
                    inst:    Instance::Composite(Composite {
                        name: "go".into(),
                        args: vec![Instance::Composite(Composite { name: "string".into(), args: vec![Instance::StringLit(StringLit("x".into()))] })],
                    }),
                    enabled: Some(false),
                }
            ])))
        );
    }

    #[test]
    fn test_parse_violation() {
        assert_eq!(
            Vec::<Violation>::from_str_head("violations:disabled action:foo()"),
            Ok(Some(("", vec![Violation::Act(ActViolation { inst: Composite { name: "foo".into(), args: vec![] } })])))
        );
        assert_eq!(
            Vec::<Violation>::from_str_head("violations:    disabled action:\n\n\nfoo()violated duty!:bar()"),
            Ok(Some(("", vec![
                Violation::Act(ActViolation { inst: Composite { name: "foo".into(), args: vec![] } }),
                Violation::Duty(DutyViolation { inst: Composite { name: "bar".into(), args: vec![] } })
            ])))
        );
        assert_eq!(
            Vec::<Violation>::from_str_head("violations:disabled action:foo()violated duty!:bar()violated invariant!:baz"),
            Ok(Some(("", vec![
                Violation::Act(ActViolation { inst: Composite { name: "foo".into(), args: vec![] } }),
                Violation::Duty(DutyViolation { inst: Composite { name: "bar".into(), args: vec![] } }),
                Violation::Invariant(InvariantViolation { name: "baz".into() })
            ])))
        );
    }

    #[test]
    fn test_parse_act_violation() {
        assert_eq!(
            ActViolation::from_str_head("disabled action: foo()"),
            Ok(Some(("", ActViolation { inst: Composite { name: "foo".into(), args: vec![] } })))
        );
        assert_eq!(ActViolation::from_str_head("disabled action: foo"), Err(Error::ExpectedInstanceAfterDisabledAction { s: "foo".into() }));
        assert_eq!(ActViolation::from_str_head("disabled actio: foo()"), Ok(None));
    }

    #[test]
    fn test_parse_duty_violation() {
        assert_eq!(
            DutyViolation::from_str_head("violated duty!: foo()"),
            Ok(Some(("", DutyViolation { inst: Composite { name: "foo".into(), args: vec![] } })))
        );
        assert_eq!(DutyViolation::from_str_head("violated duty!: foo"), Err(Error::ExpectedInstanceAfterViolatedDuty { s: "foo".into() }));
        assert_eq!(DutyViolation::from_str_head("violated duty! foo()"), Ok(None));
    }

    #[test]
    fn test_parse_invariant_violation() {
        assert_eq!(InvariantViolation::from_str_head("violated invariant!: foo"), Ok(Some(("", InvariantViolation { name: "foo".into() }))));
        assert_eq!(InvariantViolation::from_str_head("violated invariant!: foo()"), Ok(Some(("()", InvariantViolation { name: "foo".into() }))));
        assert_eq!(
            InvariantViolation::from_str_head("violated invariant!: AMY"),
            Err(Error::ExpectedTypeNameAfterViolatedInvariant { s: "AMY".into() })
        );
        assert_eq!(InvariantViolation::from_str_head("violated invariant! foo"), Ok(None));
    }



    #[test]
    fn test_parse_instance() {
        assert_eq!(Instance::from_str_head("\"Hello, world!\""), Ok(Some(("", Instance::StringLit(StringLit("Hello, world!".into()))))));
        assert_eq!(Instance::from_str_head("\"Hello, world!\\n\""), Ok(Some(("", Instance::StringLit(StringLit("Hello, world!\n".into()))))));
        assert_eq!(Instance::from_str_head("42"), Ok(Some(("", Instance::IntLit(IntLit(42))))));
        assert_eq!(Instance::from_str_head("-42"), Ok(Some(("", Instance::IntLit(IntLit(-42))))));
        assert_eq!(Instance::from_str_head("foo()"), Ok(Some(("", Instance::Composite(Composite { name: "foo".into(), args: vec![] })))));
        assert_eq!(Instance::from_str_head("+42"), Ok(None));
    }

    #[test]
    fn test_parse_string_lit() {
        assert_eq!(StringLit::from_str_head("\"Hello, world!\""), Ok(Some(("", StringLit("Hello, world!".into())))));
        assert_eq!(StringLit::from_str_head("\"Hello, world!\\n\""), Ok(Some(("", StringLit("Hello, world!\n".into())))));
        assert_eq!(StringLit::from_str_head("\"Hello, world!\" skibidi"), Ok(Some((" skibidi", StringLit("Hello, world!".into())))));
        assert_eq!(StringLit::from_str_head("\"Hello, world!"), Err(Error::UnterminatedString { s: "\"Hello, world!".into() }));
        assert_eq!(StringLit::from_str_head("Hello, world!\""), Ok(None));
    }

    #[test]
    fn test_parse_int_lit() {
        assert_eq!(IntLit::from_str_head("42"), Ok(Some(("", IntLit(42)))));
        assert_eq!(IntLit::from_str_head("-42"), Ok(Some(("", IntLit(-42)))));
        assert_eq!(IntLit::from_str_head("--42"), Ok(Some(("", IntLit(42)))));
        assert_eq!(IntLit::from_str_head("---42"), Ok(Some(("", IntLit(-42)))));
        assert_eq!(IntLit::from_str_head("42 skibidi"), Ok(Some((" skibidi", IntLit(42)))));
        assert_eq!(IntLit::from_str_head("9223372036854775807"), Ok(Some(("", IntLit(9223372036854775807)))));
        assert_eq!(IntLit::from_str_head("-9223372036854775808"), Ok(Some(("", IntLit(-9223372036854775808)))));
        assert_eq!(IntLit::from_str_head("9223372036854775808"), Err(Error::OutOfRangeInt { s: "9223372036854775808".into() }));
        assert_eq!(IntLit::from_str_head("-9223372036854775809"), Err(Error::OutOfRangeInt { s: "-9223372036854775809".into() }));
        assert_eq!(IntLit::from_str_head("98216387163871623817623817632"), Err(Error::OutOfRangeInt { s: "98216387163871623817623817632".into() }));
        assert_eq!(IntLit::from_str_head("-98216387163871623817623817632"), Err(Error::OutOfRangeInt { s: "-98216387163871623817623817632".into() }));
        assert_eq!(IntLit::from_str_head("---a"), Ok(None));
        assert_eq!(IntLit::from_str_head("\"Hello, world!\""), Ok(None));
    }

    #[test]
    fn test_parse_composite() {
        assert_eq!(Composite::from_str_head("foo()"), Ok(Some(("", Composite { name: "foo".into(), args: vec![] }))));
        assert_eq!(
            Composite::from_str_head("foo(\"bar\")"),
            Ok(Some(("", Composite { name: "foo".into(), args: vec![Instance::StringLit(StringLit("bar".into()))] })))
        );
        assert_eq!(
            Composite::from_str_head("foo(5532)"),
            Ok(Some(("", Composite { name: "foo".into(), args: vec![Instance::IntLit(IntLit(5532))] })))
        );
        assert_eq!(
            Composite::from_str_head("foo(-333)"),
            Ok(Some(("", Composite { name: "foo".into(), args: vec![Instance::IntLit(IntLit(-333))] })))
        );
        assert_eq!(
            Composite::from_str_head("foo(bar())"),
            Ok(Some(("", Composite { name: "foo".into(), args: vec![Instance::Composite(Composite { name: "bar".into(), args: vec![] })] })))
        );
        assert_eq!(
            Composite::from_str_head("foo(bar(\"baz\", quz()), 42, -88, \"howdy\", qux(\"say whhaaaa\"))"),
            Ok(Some(("", Composite {
                name: "foo".into(),
                args: vec![
                    Instance::Composite(Composite {
                        name: "bar".into(),
                        args: vec![Instance::StringLit(StringLit("baz".into())), Instance::Composite(Composite { name: "quz".into(), args: vec![] })],
                    }),
                    Instance::IntLit(IntLit(42)),
                    Instance::IntLit(IntLit(-88)),
                    Instance::StringLit(StringLit("howdy".into())),
                    Instance::Composite(Composite { name: "qux".into(), args: vec![Instance::StringLit(StringLit("say whhaaaa".into()))] })
                ],
            })))
        );
        assert_eq!(Composite::from_str_head("foo"), Ok(None));
        assert_eq!(Composite::from_str_head("foo("), Err(Error::UnterminatedParen { s: "".into() }));
        assert_eq!(Composite::from_str_head("foo(quz"), Err(Error::UnterminatedParen { s: "quz".into() }));
        assert_eq!(Composite::from_str_head("foo(quz() bar())"), Err(Error::ExpectedComma { s: " bar())".into() }));
    }



    #[test]
    fn test_parse_type_name() {
        assert_eq!(TypeName::from_str_head("foo"), Ok(Some(("", TypeName("foo".into())))));
        assert_eq!(TypeName::from_str_head("a"), Ok(Some(("", TypeName("a".into())))));
        assert_eq!(TypeName::from_str_head("camelCase"), Ok(Some(("", TypeName("camelCase".into())))));
        assert_eq!(TypeName::from_str_head("kebab-case"), Ok(Some(("", TypeName("kebab-case".into())))));
        assert_eq!(TypeName::from_str_head("snake_case"), Ok(Some(("", TypeName("snake_case".into())))));
        assert_eq!(TypeName::from_str_head("mixCase-es_es"), Ok(Some(("", TypeName("mixCase-es_es".into())))));
        assert_eq!(
            TypeName::from_str_head("[everything goes in <> square BRACKAETS]"),
            Ok(Some(("", TypeName("[everything goes in <> square BRACKAETS]".into()))))
        );
        assert_eq!(
            TypeName::from_str_head("<everything goes in [] triangular BRACKAETS>"),
            Ok(Some(("", TypeName("<everything goes in [] triangular BRACKAETS>".into()))))
        );
        assert_eq!(TypeName::from_str_head("[[nested brackets]]"), Ok(Some(("", TypeName("[[nested brackets]]".into())))));
        assert_eq!(TypeName::from_str_head("<<nested brackets>>"), Ok(Some(("", TypeName("<<nested brackets>>".into())))));
        assert_eq!(TypeName::from_str_head("Foo"), Ok(None));
        assert_eq!(TypeName::from_str_head("[unterminated"), Err(Error::UnterminatedDelim { delim: ']', s: "[unterminated".into() }));
        assert_eq!(TypeName::from_str_head("[[unterminated]"), Err(Error::UnterminatedDelim { delim: ']', s: "[[unterminated]".into() }));
        assert_eq!(TypeName::from_str_head("<unterminated"), Err(Error::UnterminatedDelim { delim: '>', s: "<unterminated".into() }));
        assert_eq!(TypeName::from_str_head("<<unterminated>"), Err(Error::UnterminatedDelim { delim: '>', s: "<<unterminated>".into() }));
    }
}
