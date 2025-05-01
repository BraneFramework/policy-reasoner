//  HASH.rs
//    by Lut99
//
//  Created:
//    01 May 2025, 14:33:06
//  Last edited:
//    01 May 2025, 15:24:08
//  Auto updated?
//    Yes
//
//  Description:
//!   Provides functions to compute hashes of eFLINT specs.
//

use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::{Display, Formatter, Result as FResult};
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncReadExt as _;
use tracing::{Level, debug, span};


/***** ERRORS *****/
/// Formats a list but like, prettily.
struct PrettyPathListFormatter<'l>(&'l [PathBuf]);
impl<'l> Display for PrettyPathListFormatter<'l> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        if self.0.is_empty() {
            return write!(f, "<none>");
        }
        for (i, path) in self.0.into_iter().enumerate() {
            if i > 0 && i < self.0.len() - 1 {
                write!(f, ", ")?;
            } else if i > 0 {
                write!(f, " or ")?;
            }
            write!(f, "{:?}", path.display())?;
        }
        Ok(())
    }
}

/// Errors emitted by [`compute_policy_hash()`].
#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to open file {:?}", path.display())]
    FileOpen { path: PathBuf, source: std::io::Error },
    #[error("Failed to read from file {:?}", path.display())]
    FileRead { path: PathBuf, source: std::io::Error },
    #[error("Failed to find dependency {path:?} in {}", PrettyPathListFormatter(include_dirs))]
    ImportNotFound { path: String, include_dirs: Vec<PathBuf> },
}





/***** HELPERS *****/
/// Represents which of the two import types we've seen.
enum Import {
    Include,
    Require,
}
impl Import {
    /// Returns the keyword we're parsing.
    #[inline]
    pub const fn keyword(&self) -> &'static [u8] {
        match self {
            Self::Include => b"#include",
            Self::Require => b"#require",
        }
    }
}

/// Defines the parsing states.
enum State {
    /// Waiting for the pound `#` symbol.
    Pound,
    /// Waiting to see if the pound symbol is followed by `i` or `r`.
    IncludeOrRequire,
    /// Parsing the full `#include`- or `#require`-keyword. The number indicates the index up to which point we've already parsed.
    Import(Import, usize),
    /// We're parsing the keyword. Now parse the start of the path string.
    PathStart(Import),
    /// We're now actively parsing the path.
    Path(Import, Vec<u8>),
    /// We've parsed the path. Just the dot to go now.
    Dot(Import, Vec<u8>),
}





/***** IMPLEMENTATION *****/
/// Does the heavy-lifting of [`compute_policy_hash()`].
async fn compute_policy_hash_of(
    mut handle: File,
    path: &Path,
    base_path: &Path,
    include_dirs: &[&Path],
    included: &mut HashSet<PathBuf>,
    res: &mut Sha256,
) -> Result<(), Error> {
    debug!("Computing policy hash of {:?}", path.display());

    // Go through the file chunk-by-chunk
    let mut state = State::Pound;
    let mut buf: [u8; 2048] = [0; 2048];
    loop {
        // Read a chunk
        let buf_len: usize = handle.read(&mut buf).await.map_err(|source| Error::FileRead { path: path.into(), source })?;
        if buf_len == 0 {
            return Ok(());
        }

        // Evaluate it
        let mut i: usize = 0;
        while i < buf_len {
            let b: u8 = buf[i];
            match state {
                State::Pound if b == b'#' => {
                    i += 1;
                    state = State::IncludeOrRequire;
                },
                State::Pound => {
                    res.write_all(&[b]).unwrap();
                    i += 1;
                },

                State::IncludeOrRequire if b == b'i' => {
                    i += 1;
                    state = State::Import(Import::Include, 2);
                },
                State::IncludeOrRequire if b == b'r' => {
                    i += 1;
                    state = State::Import(Import::Require, 2);
                },
                State::IncludeOrRequire => {
                    // Write the `#` we memorized, then try this byte again
                    res.write_all(&[b'#']).unwrap();
                    state = State::Pound;
                },

                // Either we've 1) completed, 2) found an correct char or 3) found an incorrect char.
                State::Import(imp, j) if j >= imp.keyword().len() => {
                    // Don't increment, this byte may be the start already
                    state = State::PathStart(imp);
                },
                State::Import(imp, j) if j < imp.keyword().len() && b == imp.keyword()[j] => {
                    i += 1;
                    state = State::Import(Import::Include, j + 1);
                },
                State::Import(imp, j) => {
                    // Write that which we've parsed, then try this byte as if fresh
                    res.write_all(&imp.keyword()[..j]).unwrap();
                    state = State::Pound;
                },

                State::PathStart(imp) if b == b'"' => {
                    i += 1;
                    state = State::Path(imp, Vec::with_capacity(32));
                },
                State::PathStart(imp) if (b as char).is_whitespace() => {
                    // These we skip idly. We don't even write the hash, as it's idle space AND we don't want to accidentally mix up the order if the next byte reveals we were wrong.
                    i += 1;
                    state = State::PathStart(imp);
                },
                State::PathStart(imp) => {
                    // Not it after all
                    res.write_all(imp.keyword()).unwrap();
                    state = State::Pound;
                },

                // Note: we don't escape these strings
                State::Path(imp, mut path) if b != b'"' => {
                    path.push(b);
                    i += 1;
                    state = State::Path(imp, path);
                },
                State::Path(imp, path) => {
                    // Just wait for the dot!
                    i += 1;
                    state = State::Dot(imp, path);
                },

                State::Dot(imp, path) if b == b'.' => {
                    // We've successfully parsed a chunk! Let's get the path as a path
                    let path: Cow<str> = String::from_utf8_lossy(&path);
                    let path: &Path = <str as AsRef<Path>>::as_ref(path.as_ref());

                    // Include the file if it's an include OR we've never included it before.
                    if matches!(imp, Import::Include) || !included.contains(path) {
                        Box::pin(compute_policy_hash_of(
                            File::open(path).await.map_err(|source| Error::FileOpen { path: path.into(), source })?,
                            path,
                            base_path,
                            include_dirs,
                            included,
                            res,
                        ))
                        .await?;
                    }
                    included.insert(path.into());

                    // Now continue as before with the next byte
                    i += 1;
                    state = State::Pound;
                },
                State::Dot(imp, path) if (b as char).is_whitespace() => {
                    // Idly skip
                    i += 1;
                    state = State::Dot(imp, path);
                },
                State::Dot(imp, path) => {
                    // Not it AFTER ALL
                    res.write_all(imp.keyword()).unwrap();
                    res.write_all(&[b'"']).unwrap();
                    res.write_all(&path).unwrap();
                    res.write_all(&[b'"']).unwrap();
                    state = State::Pound;
                },
            }
        }
    }
}





/***** LIBRARY *****/
/// Recursively computes the hash of the given eFLINT file.
///
/// This is non-trivial as any imports will have to be chased.
///
/// # Arguments
/// - `path`: The path to the eFLINT file to hash (which, in turn, specifies the dependencies).
/// - `include_dirs`: Any additional include directories to use for the search. By default, the
///   current working directory, the directory of the given file and the directory of the currently
///   recursed file are included.
///
/// # Returns
/// The hash of the policy, as a 256-bit byte array.
///
/// # Errors
/// This function may error if we failed to open the given `path` as a file, or failed to find any
/// of the (recursive) dependencies.
pub async fn compute_policy_hash(path: impl AsRef<Path>, include_dirs: &[&Path]) -> Result<[u8; 32], Error> {
    let path: &Path = path.as_ref();
    let _span = span!(Level::DEBUG, "compute_policy_hash", file = path.display().to_string());

    // We use the excellent Write impl of `Sha256`
    let mut hasher = Sha256::new();
    compute_policy_hash_of(
        File::open(path).await.map_err(|source| Error::FileOpen { path: path.into(), source })?,
        path,
        path,
        &include_dirs,
        &mut HashSet::new(),
        &mut hasher,
    )
    .await?;
    Ok(hasher.finalize().into())
}
