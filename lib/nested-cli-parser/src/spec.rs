//  SPEC.rs
//    by Lut99
//
//  Created:
//    18 Jan 2024, 16:06:11
//  Last edited:
//    07 Feb 2024, 18:02:27
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the interface for the implementations provided in this
//!   crate.
//

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};

/***** FORMATTERS *****/
/// Formats any given [`NestedCliParser`].
pub struct NestedCliParserHelpFormatter<'n, 'l, P> {
    /// A name for whatever we're parsing.
    name:   &'n str,
    /// A shortname for the argument that contains the nested arguments we parse.
    short:  char,
    /// A longname for the argument that contains the nested arguments we parse.
    long:   &'l str,
    /// The parser in question.
    parser: P,
}
impl<P: NestedCliParser> Display for NestedCliParserHelpFormatter<'_, '_, P> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { self.parser.help_fmt(self.name, self.short, self.long, f) }
}

/***** LIBRARY *****/
/// Defines that a particular type can parse nested CLI commands.
///
/// The commands should be given as a string, which the parser then extracts as arguments.
pub trait NestedCliParser {
    /// The associated state that is parsed from the nested argument list.
    type Args;
    /// Any error that is thrown when parsing.
    type ParseError: Error;

    /// Formats the help string of the NestedCliParser.
    ///
    /// You typically don't call this method yourself, but instead use [`NestedCliParser::help()`] to call it for you through [`Display`].
    ///
    /// # Arguments
    /// - `name`: Some name of whatever we're parsing.
    /// - `short`: A shortname for the argument that contains the nested arguments we parse.
    /// - `long`: A longname for the argument that contains the nested arguments we parse.
    /// - `f`: Some [`Formatter`] to which to write the help string.
    ///
    /// # Errors
    /// This function errors if it failed to write to the given `f`ormatter.
    fn help_fmt(&self, name: &str, short: char, long: &str, f: &mut Formatter<'_>) -> FResult;
    /// Returns a formatter that can be used to [`Display`] the help string for this NestedCliParser.
    ///
    /// In contrast to [`NestedCliParser::help()`], this one consumes the parser to avoid the lifetime dependency.
    ///
    /// # Arguments
    /// - `name`: Some name of whatever we're parsing.
    /// - `short`: A shortname for the argument that contains the nested arguments we parse.
    /// - `long`: A longname for the argument that contains the nested arguments we parse.
    ///
    /// # Returns
    /// A [`NestedCliParserHelpFormatter`] that calls [`NestedCliParser::help_fmt()`] for the formatter to which it is applied.
    #[inline]
    fn into_help<'n, 'l>(self, name: &'n str, short: char, long: &'l str) -> NestedCliParserHelpFormatter<'n, 'l, Self>
    where
        Self: Sized,
    {
        NestedCliParserHelpFormatter { name, short, long, parser: self }
    }
    /// Returns a formatter that can be used to [`Display`] the help string for this NestedCliParser.
    ///
    /// # Arguments
    /// - `name`: Some name of whatever we're parsing.
    /// - `short`: A shortname for the argument that contains the nested arguments we parse.
    /// - `long`: A longname for the argument that contains the nested arguments we parse.
    ///
    /// # Returns
    /// A [`NestedCliParserHelpFormatter`] that calls [`NestedCliParser::help_fmt()`] for the formatter to which it is applied.
    #[inline]
    fn help<'s, 'n, 'l>(&'s self, name: &'n str, short: char, long: &'l str) -> NestedCliParserHelpFormatter<'n, 'l, &'s Self> {
        NestedCliParserHelpFormatter { name, short, long, parser: self }
    }

    /// Parses the given string as a set of nested CLI arguments.
    ///
    /// # Arguments
    /// - `args`: The raw [`&str`] to parse.
    ///
    /// # Returns
    /// A new instance of [`Self::Args`] that contains the information parsed from the raw `args`.
    ///
    /// # Errors
    /// This function has free roam to error as it desires. Typically, however, this should be when the input is invalid for this parser.
    fn parse(&self, args: &str) -> Result<Self::Args, Self::ParseError>;
}
