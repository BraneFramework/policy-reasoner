//  HASH.rs
//    by Lut99
//
//  Created:
//    01 May 2025, 14:33:06
//  Last edited:
//    06 May 2025, 12:53:18
//  Auto updated?
//    Yes
//
//  Description:
//!   Provides functions to compute hashes of eFLINT specs.
//

use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use share::formatters::PathListFormatter;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt as _};
use tracing::{debug, instrument};


/// Errors emitted by [`compute_policy_hash()`].
#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to open file {path}", path = path.display())]
    FileOpen { path: PathBuf, source: std::io::Error },
    #[error("Failed to read from file {path}", path = path.display())]
    FileRead { path: PathBuf, source: std::io::Error },
    #[error("Failed to get current working directory")]
    GetCwd { source: std::io::Error },
    #[error("Failed to find dependency {path} as {import_paths}", path = path.display(), import_paths = PathListFormatter::language_or(imppaths))]
    ImportNotFound { path: PathBuf, imppaths: Vec<PathBuf> },
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
    /// We've parsed the keyword. Now parse the start of the path string.
    PathStart,
    /// We're now actively parsing the path.
    Path(Vec<u8>),
    /// We've parsed the path. Just the dot to go now.
    Dot(Vec<u8>),
}





/***** IMPLEMENTATION *****/
/// Does the heavy-lifting of [`compute_policy_hash()`].
async fn find_deps_of(mut handle: File, path: &Path, base_path: &Path, include_dirs: &[&Path], files: &mut HashSet<PathBuf>) -> Result<(), Error> {
    debug!("Searching for dependencies of eFLINT file {path}", path = path.display());

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
                    // Try this byte again
                    state = State::Pound;
                },

                // Either we've 1) completed, 2) found an correct char or 3) found an incorrect char.
                State::Import(imp, j) if j >= imp.keyword().len() => {
                    // Don't increment, this byte may be the start already
                    state = State::PathStart;
                },
                State::Import(imp, j) if j < imp.keyword().len() && b == imp.keyword()[j] => {
                    i += 1;
                    state = State::Import(imp, j + 1);
                },
                State::Import(_, _) => {
                    // Try this byte as if it's afresh
                    state = State::Pound;
                },

                State::PathStart if b == b'"' => {
                    i += 1;
                    state = State::Path(Vec::with_capacity(32));
                },
                State::PathStart if (b as char).is_whitespace() => {
                    // These we skip idly. We don't even write the hash, as it's idle space AND we don't want to accidentally mix up the order if the next byte reveals we were wrong.
                    i += 1;
                    state = State::PathStart;
                },
                State::PathStart => {
                    // Not it after all
                    state = State::Pound;
                },

                // Note: we don't escape these strings
                State::Path(mut imppath) if b != b'"' => {
                    imppath.push(b);
                    i += 1;
                    state = State::Path(imppath);
                },
                State::Path(imppath) => {
                    // Just wait for the dot!
                    i += 1;
                    state = State::Dot(imppath);
                },

                State::Dot(imppath) if b == b'.' => {
                    // We've successfully parsed a chunk! Let's get the path as a path
                    let imppath: Cow<str> = String::from_utf8_lossy(&imppath);
                    let imppath: &Path = <str as AsRef<Path>>::as_ref(imppath.as_ref());

                    // Attempt to resolve the path if it's relative
                    let imppaths: Vec<Cow<Path>> = if imppath.is_relative() {
                        let mut imppaths = Vec::with_capacity(3 + include_dirs.len());
                        imppaths.push(Cow::Owned(std::env::current_dir().map_err(|source| Error::GetCwd { source })?.join(imppath)));
                        if let Some(parent) = base_path.parent() {
                            imppaths.push(Cow::Owned(parent.join(imppath)));
                        }
                        if let Some(parent) = path.parent() {
                            imppaths.push(Cow::Owned(parent.join(imppath)));
                        }
                        for dir in include_dirs {
                            imppaths.push(Cow::Owned(dir.join(imppath)));
                        }
                        imppaths
                    } else {
                        vec![Cow::Borrowed(imppath)]
                    };

                    // Recurse & then update the dependency list
                    let mut found: bool = false;
                    for imppath in &imppaths {
                        // Ensure it exists
                        if !imppath.exists() {
                            continue;
                        }
                        found = true;

                        // If we haven't added it yet, add it
                        if !files.contains(imppath.as_ref()) {
                            files.insert(imppath.clone().into_owned());
                            Box::pin(find_deps_of(
                                File::open(imppath).await.map_err(|source| Error::FileOpen { path: imppath.clone().into_owned(), source })?,
                                imppath.as_ref(),
                                base_path,
                                include_dirs,
                                files,
                            ))
                            .await?;
                        }
                        break;
                    }
                    if !found {
                        return Err(Error::ImportNotFound {
                            path:     imppath.into(),
                            imppaths: imppaths.into_iter().map(Cow::into_owned).collect(),
                        });
                    }

                    // Now continue as before with the next byte
                    i += 1;
                    state = State::Pound;
                },
                State::Dot(imppath) if (b as char).is_whitespace() => {
                    // Idly skip
                    i += 1;
                    state = State::Dot(imppath);
                },
                State::Dot(_) => {
                    // Not it AFTER ALL
                    state = State::Pound;
                },
            }
        }
    }
}





/***** LIBRARY *****/
/// Finds all dependencies of the given eFLINT file.
///
/// This will read the file, search for `#include`- and `#require`-directives, and then recursively
/// search those files until one set of files is returned.
///
/// # Arguments
/// - `path`: The path to the eFLINT file to hash (which, in turn, specifies the dependencies).
/// - `include_dirs`: Any additional include directories to use for the search. By default, the
///   current working directory, the directory of the given file and the directory of the currently
///   recursed file are included.
///
/// # Returns
/// A [`HashSet`] of [`PathBuf`]s encoding the found files. This includes the given `path`.
///
/// # Errors
/// This function may error if we failed to open the given `path` as a file, or failed to find any
/// of the (recursive) dependencies.
#[instrument(skip_all, fields(file=%path.as_ref().display()))]
pub async fn find_deps(path: impl AsRef<Path>, include_dirs: &[&Path]) -> Result<HashSet<PathBuf>, Error> {
    let path: &Path = path.as_ref();

    // Delegate to the recursive function
    let mut res: HashSet<PathBuf> = HashSet::with_capacity(16);
    find_deps_of(File::open(path).await.map_err(|source| Error::FileOpen { path: path.into(), source })?, path, path, include_dirs, &mut res).await?;

    // Done
    Ok(res)
}



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
    // Find the set of all files first
    let files = find_deps(path, include_dirs).await?;

    // Order them
    let mut files: Vec<PathBuf> = files.into_iter().collect();
    files.sort();

    // Now hash all of the files
    let mut hasher = Sha256::new();
    for file in files {
        // Open the file
        debug!("Hashing eFLINT file {}", file.display());
        let handle = File::open(&file).await.map_err(|source| Error::FileOpen { path: file.clone(), source })?;
        hash_async_reader(&mut hasher, handle).await.map_err(|source| Error::FileRead { path: file.clone(), source })?;
    }

    // Done
    Ok(hasher.finalize().into())
}

async fn hash_async_reader(hasher: &mut impl Digest, mut reader: impl AsyncRead + Unpin) -> std::io::Result<()> {
    let mut buf = [0_u8; 2 ^ 14];
    loop {
        // Read a chunk
        let buf_len: usize = reader.read(&mut buf).await?;
        if buf_len == 0 {
            break;
        }
        // Write it to the hasher
        hasher.update(&buf[..buf_len]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;


    #[tokio::test]
    async fn test_hash_empty_reader() {
        let mut hasher = Sha256::new();
        let reader = Cursor::new(Vec::<u8>::new());

        let result = hash_async_reader(&mut hasher, reader).await;
        assert!(result.is_ok());

        let hash = hasher.finalize();
        // SHA256 of empty input
        let expected = hex::decode("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855").unwrap();
        assert_eq!(hash.as_slice(), expected.as_slice());
    }

    #[tokio::test]
    async fn test_hash_small_data() {
        let mut hasher = Sha256::new();
        let data = b"hello world";
        let reader = Cursor::new(data.to_vec());

        let result = hash_async_reader(&mut hasher, reader).await;
        assert!(result.is_ok());

        let hash = hasher.finalize();
        // SHA256 of "hello world"
        let expected = hex::decode("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9").unwrap();
        assert_eq!(hash.as_slice(), expected.as_slice());
    }

    #[tokio::test]
    async fn test_hash_large_data() {
        let mut hasher = Sha256::new();
        // Create data larger than buffer size (16384 bytes)
        let data = vec![0x42u8; 20000];
        let reader = Cursor::new(data.clone());

        let result = hash_async_reader(&mut hasher, reader).await;
        assert!(result.is_ok());

        let hash = hasher.finalize();

        // Compare with direct hashing
        let mut expected_hasher = Sha256::new();
        expected_hasher.update(&data);
        // let expected = expected_hasher.finalize();

        let expected = hex::decode("c4f984e0cf8a5d4a8f60c5d2d33848e4772045ba667a4e52851a7dd7eea6d6e2").unwrap();
        assert_eq!(hash.as_slice(), expected.as_slice());
    }

    #[tokio::test]
    async fn test_hash_exact_buffer_size() {
        let mut hasher = Sha256::new();
        // Create data exactly buffer size (16384 bytes)
        let data = vec![0x37u8; 16384];
        let reader = Cursor::new(data.clone());

        let result = hash_async_reader(&mut hasher, reader).await;
        assert!(result.is_ok());

        let hash = hasher.finalize();

        let expected = hex::decode("f3f59afe21c99e2471439dbdda0cf583f296ebe6995f8e3d3dac23a87d6afe78").unwrap();
        assert_eq!(hash.as_slice(), expected.as_slice());
    }
}
