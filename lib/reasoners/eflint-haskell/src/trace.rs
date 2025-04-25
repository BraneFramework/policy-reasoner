//  TRACE.rs
//    by Lut99
//
//  Created:
//    17 Apr 2025, 00:06:39
//  Last edited:
//    25 Apr 2025, 10:15:44
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

use thiserror::Error;


/***** ERRORS *****/
/// Defines fatal parsing errors for parsing traces [from strings](FromStrHead::from_str_head()).
#[derive(Debug, Error, Eq, PartialEq)]
pub enum Error {
    #[error("Expected a comma at {s:?}")]
    ExpectedComma { s: String },
    #[error("Out-of-range integer at {s:?}")]
    OutOfRangeInt { s: String },
    #[error("Failed to parse instance following postulation op {op} at {s:?}")]
    PostulationOpWithoutInstance { op: PostulationOp, s: String },
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
/// Defines a postulation delta.
///
/// This means that a new fact has been created, terminated or obfuscated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Postulation {
    /// The operation applied.
    pub op:   PostulationOp,
    /// The instance that was postulated.
    pub inst: Instance,
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
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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



/// Defines an instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Instance {
    /// A naked string literal.
    StringLit(StringLit),
    /// A naked int literal.
    IntLit(IntLit),
    /// A composite type.
    Composite(Composite),
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringLit(pub String);
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntLit(pub i64);
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
            if c >= '0' && c <= '9' {
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
        if seen_one {
            return Ok(Some((&s[s.len()..], IntLit(value))));
        } else {
            return Ok(None);
        }
    }
}

/// Defines a composite type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Composite {
    /// The name of the type.
    pub name: String,
    /// The arguments of the type.
    pub args: Vec<Instance>,
}
impl FromStrHead for Composite {
    type Error = Error;

    #[inline]
    fn from_str_head(s: &str) -> Result<Option<(&str, Self)>, Self::Error> {
        // Parse an identifier type first
        let (rem, name): (&str, String) = match TypeName::from_str_head(s).unwrap() {
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
            return Ok(Some((&rem[1..], Self { name, args })));
        } else {
            Err(Error::UnterminatedParen { s: rem.into() })
        }
    }
}



/// Parses an eFLINT type name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeName(pub String);
impl FromStrHead for TypeName {
    type Error = Infallible;

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
                if (c >= 'a' && c <= 'z') || c == '_' || c == '-' {
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
                if (c < 'a' || c > 'z') && (c < 'A' || c > 'Z') && c != '-' && c != '_' {
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
        Ok(None)
    }
}





/***** TESTS *****/
#[cfg(test)]
mod tests {
    use super::*;

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
}
