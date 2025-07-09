//  LIB.rs
//    by Lut99
//
//  Created:
//    30 Nov 2023, 10:38:50
//  Last edited:
//    11 Oct 2024, 16:10:25
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines a high-level wrapper around Olaf's
//!   [`eflint-to-json`](https://github.com/Olaf-Erkemeij/eflint-server)
//!   executable that compiles eFLINT to eFLINT JSON Specification.
//

// Declare modules
pub mod download;

use std::borrow::Cow;
use std::collections::HashSet;
use std::error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::fs::{self, File, Permissions};
use std::io::{BufRead as _, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};

use console::Style;
#[cfg(feature = "async-tokio")]
use tokio::fs::{self as tfs, File as TFile};
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt, AsyncWriteExt as _, BufReader as TBufReader};
#[cfg(feature = "async-tokio")]
use tokio::process::{ChildStdin as TChildStdin, ChildStdout as TChildStdout, Command as TCommand};
use tracing::{debug, info};

#[cfg(feature = "async-tokio")]
use crate::download::download_file_async;
use crate::download::{DownloadSecurity, download_file};


/***** CONSTANTS *****/
/// Compiler download URL.
const COMPILER_URL: &str = "https://github.com/Olaf-Erkemeij/eflint-server/raw/bd3997df89441f13cbc82bd114223646df41540d/eflint-to-json";
/// Compiler download checksum.
const COMPILER_CHECKSUM: [u8; 32] = hex_literal::hex!("4e4e59b158ca31e532ec0a22079951788696ffa5d020b36790b4461dbadec83d");





/***** ERRORS *****/
/// Defines a wrapper around multiple streams.
#[derive(Debug)]
pub struct ChildStreams(Vec<ChildStream>);
impl Display for ChildStreams {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        for stream in &self.0 {
            writeln!(f, "{stream}")?;
            writeln!(f)?;
        }
        Ok(())
    }
}
impl error::Error for ChildStreams {}

/// Defines a wrapper around [`ChildStdout`]/[`std::process::ChildStderr`] that allow them to be serialized as errors in a trace.
#[derive(Debug)]
pub struct ChildStream(&'static str, String);
impl ChildStream {
    /// Constructor for the ChildStream.
    ///
    /// # Arguments
    /// - `what`: The thing we're wrapping (e.g., `stdout`).
    /// - `stream`: The stream(-like) to wrap the contents of.
    ///
    /// # Returns
    /// A new ChildStream that either has the stream's contents, or some message saying the contents couldn't be retrieved.
    fn new(what: &'static str, mut stream: impl Read) -> Self {
        // Attempt to read it all
        let mut buf: String = String::new();
        match stream.read_to_string(&mut buf) {
            Ok(_) => Self(what, buf),
            Err(err) => Self(what, format!("<failed to read stream: {err}>")),
        }
    }

    /// Constructor for the ChildStream for async streams.
    ///
    /// # Arguments
    /// - `what`: The thing we're wrapping (e.g., `stdout`).
    /// - `stream`: The stream(-like) to wrap the contents of.
    ///
    /// # Returns
    /// A new ChildStream that either has the stream's contents, or some message saying the contents couldn't be retrieved.
    #[cfg(feature = "async-tokio")]
    async fn new_async(what: &'static str, mut stream: impl AsyncReadExt + Unpin) -> Self {
        // Attempt to read it all
        let mut buf: String = String::new();
        match stream.read_to_string(&mut buf).await {
            Ok(_) => Self(what, buf),
            Err(err) => Self(what, format!("<failed to read stream: {err}>")),
        }
    }
}
impl Display for ChildStream {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        // Write it over multiple lines
        writeln!(f, "{}:", self.0)?;
        writeln!(f, "{}", (0..80).map(|_| '-').collect::<String>())?;
        writeln!(f, "{}", self.1)?;
        writeln!(f, "{}", (0..80).map(|_| '-').collect::<String>())?;
        Ok(())
    }
}
impl error::Error for ChildStream {}



/// Defines toplevel errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The child failed
    #[error("Child process {cmd:?} failed with exit status {status}")]
    ChildFailed { cmd: String, status: ExitStatus, output: ChildStreams },
    /// Failed to read from child stdout.
    #[error("Failed to read from child stdin")]
    ChildRead { source: std::io::Error },
    /// Failed to wait for the child to be ready.
    #[error("Failed to wait for child")]
    ChildWait { source: std::io::Error },
    /// Failed to write to child stdin.
    #[error("Failed to write to child stdin")]
    ChildWrite { source: std::io::Error },
    /// Failed to download the compiler.
    ///
    /// NOTE: `err` is boxed to not make this variant much larger in memory than the rest.
    #[error("Failed to download 'eflint-to-json' compiler from '{}' to '{}'", from, to.display())]
    CompilerDownload { from: String, to: PathBuf, source: Box<crate::download::Error> },
    /// Failed to create the output file.
    #[error("Failed to create output file '{}'", path.display())]
    FileCreate { path: PathBuf, source: std::io::Error },
    /// Failed to get metadata of file.
    #[error("Failed to get metadata of file '{}'", path.display())]
    FileMetadata { path: PathBuf, source: std::io::Error },
    /// Failed to open the input file.
    #[error("Failed to open input file '{}'", path.display())]
    FileOpen { path: PathBuf, source: std::io::Error },
    /// Failed to set permissions of file.
    #[error("Failed to set permissions of file '{}'", path.display())]
    FilePermissions { path: PathBuf, source: std::io::Error },
    /// Failed to read the input file.
    #[error("Failed to read from input file '{}'", path.display())]
    FileRead { path: PathBuf, source: std::io::Error },
    /// Failed to open included file.
    #[error("Failed to open included file '{}' (in file '{}')", path.display(), parent.display())]
    IncludeOpen { parent: PathBuf, path: PathBuf, source: std::io::Error },
    /// Missing a quote in the `#include`-string.
    #[error("Missing quotes (\") in '{raw}' (in file '{}')", parent.display())]
    MissingQuote { parent: PathBuf, raw: String },
    /// Failed to canonicalize the given path.
    #[error("Failed to canonicalize path '{}' (in file '{}')", path.display(), parent.display())]
    PathCanonicalize { parent: PathBuf, path: PathBuf, source: std::io::Error },
    /// Failed to spawn the eflint-to-json compiler process.
    #[error("Failed to spawn command {cmd:?}")]
    Spawn { cmd: String, source: std::io::Error },
    /// Failed to write to the output writer.
    #[error("Failed to write to output writer")]
    WriterWrite { source: std::io::Error },
}





/***** HELPER FUNCTIONS *****/
/// Analyses a potential `#input(...)` or `#require(...)` line from eFLINT.
///
/// # Arguments
/// - `imported`: The set of already imported files (relevant for require).
/// - `path`: The path of the current file.
/// - `line`: The parsed line.
///
/// # Returns
/// A handle to the included file (as a tuple of the path + the handle) if any, or else [`None`].
///
/// # Errors
/// This function can error if we failed to open the included file.
fn potentially_include(imported: &mut HashSet<PathBuf>, path: &Path, line: &str) -> Result<Option<Option<(PathBuf, File)>>, Error> {
    // Strip whitespace
    let line: &str = line.trim();

    // Check it's a line
    if !line.starts_with("#include") && !line.starts_with("#require") || line.chars().last().map(|c| c != '.').unwrap_or(true) {
        return Ok(None);
    }

    // Extract the text
    let squote: usize = line.find('"').ok_or_else(|| Error::MissingQuote { parent: path.into(), raw: line.into() })?;
    let equote: usize = line.rfind('"').ok_or_else(|| Error::MissingQuote { parent: path.into(), raw: line.into() })?;
    let incl_path: PathBuf = PathBuf::from(&line[squote + 1..equote]);

    // Build the path
    let parent: Option<&Path> = path.parent();
    // NOTE: Allowing the `is_none()`, `unwrap()` because else we ruin the logic
    #[allow(clippy::unnecessary_unwrap)]
    let incl_path: PathBuf = if incl_path.is_absolute() || parent.is_none() { incl_path } else { parent.unwrap().join(incl_path) };
    let incl_path: PathBuf =
        incl_path.canonicalize().map_err(|source| Error::PathCanonicalize { parent: path.into(), path: incl_path.clone(), source })?;

    // Check if we've seen this before if it's require
    if line.starts_with("#require") && imported.contains(&incl_path) {
        return Ok(Some(None));
    }
    imported.insert(incl_path.clone());

    // Build the path and attempt to open it
    let handle = File::open(&incl_path).map_err(|source| Error::IncludeOpen { parent: path.into(), path: incl_path.clone(), source })?;

    // OK
    Ok(Some(Some((incl_path, handle))))
}

/// Analyses a potential `#input(...)` or `#require(...)` line from eFLINT.
///
/// # Arguments
/// - `imported`: The set of already imported files (relevant for require).
/// - `path`: The path of the current file.
/// - `line`: The parsed line.
///
/// # Returns
/// A handle to the included file (as a tuple of the path + the handle) if any, or else [`None`].
///
/// # Errors
/// This function can error if we failed to open the included file.
#[cfg(feature = "async-tokio")]
async fn potentially_include_async(imported: &mut HashSet<PathBuf>, path: &Path, line: &str) -> Result<Option<Option<(PathBuf, TFile)>>, Error> {
    // Strip whitespace
    let line: &str = line.trim();

    // Check it's a line
    if !line.starts_with("#include") && !line.starts_with("#require") || line.chars().last().map(|c| c != '.').unwrap_or(true) {
        return Ok(None);
    }

    // Extract the text
    let squote: usize = line.find('"').ok_or_else(|| Error::MissingQuote { parent: path.into(), raw: line.into() })?;
    let equote: usize = line.rfind('"').ok_or_else(|| Error::MissingQuote { parent: path.into(), raw: line.into() })?;
    let incl_path: PathBuf = PathBuf::from(&line[squote + 1..equote]);

    // Build the path
    let parent: Option<&Path> = path.parent();
    // NOTE: Allowing the `is_none()`, `unwrap()` because else we ruin the logic
    #[allow(clippy::unnecessary_unwrap)]
    let incl_path: PathBuf = if incl_path.is_absolute() || parent.is_none() { incl_path } else { parent.unwrap().join(incl_path) };
    let incl_path: PathBuf =
        tfs::canonicalize(&incl_path).await.map_err(|source| Error::PathCanonicalize { parent: path.into(), path: incl_path, source })?;

    // Check if we've seen this before if it's require
    if line.starts_with("#require") && imported.contains(&incl_path) {
        return Ok(Some(None));
    }
    imported.insert(incl_path.clone());

    // Build the path and attempt to open it
    let handle = TFile::open(&incl_path).await.map_err(|source| Error::IncludeOpen { parent: path.into(), path: incl_path.clone(), source })?;

    // OK
    Ok(Some(Some((incl_path, handle))))
}

/// Streams the given file's contents to the stdin of the given process, including files as necessary halfway.
///
/// # Arguments
/// - `imported`: The set of already imported files (relevant for require).
/// - `path`: The path of the file we're currently importing. Only used for debugging purposes.
/// - `handle`: Handle to the [`File`] we're going to read.
/// - `child`: The [`ChildStdin`] to write the stream of input files to.
///
/// # Errors
/// This function may error if we at any point failed to open/read a file, found `#include`s or `#require`s pointing to non-existant files or if we could not write to the `child`.
fn load_input(imported: &mut HashSet<PathBuf>, path: &Path, handle: BufReader<File>, child: &mut ChildStdin) -> Result<(), Error> {
    debug!("Importing file '{}'", path.display());

    // Read the lines for the file
    for line in handle.lines() {
        // Unwrap the line
        let line: String = line.map_err(|source| Error::FileRead { path: path.into(), source })?;

        // See if a file is included
        match potentially_include(imported, path, &line)? {
            Some(Some((child_path, child_handle))) => {
                load_input(imported, &child_path, BufReader::new(child_handle), child)?;
            },
            // We don't want to write the line since we already imported it
            Some(None) => {},
            None => {
                child.write_all(line.as_bytes()).map_err(|source| Error::ChildWrite { source })?;
                child.write_all(b"\n").map_err(|source| Error::ChildWrite { source })?;
            },
        }
    }

    // Done!
    Ok(())
}

/// Streams the given file's contents to the stdin of the given process, including files as necessary halfway.
///
/// # Arguments
/// - `imported`: The set of already imported files (relevant for require).
/// - `path`: The path of the file we're currently importing. Only used for debugging purposes.
/// - `handle`: Handle to the [`TFile`]we're going to read.
/// - `child`: The [`TChildStdin`] to write the stream of input files to.
///
/// # Errors
/// This function may error if we at any point failed to open/read a file, found `#include`s or `#require`s pointing to non-existant files or if we could not write to the `child`.
#[cfg(feature = "async-tokio")]
#[async_recursion::async_recursion]
async fn load_input_async(imported: &mut HashSet<PathBuf>, path: &Path, handle: TBufReader<TFile>, child: &mut TChildStdin) -> Result<(), Error> {
    debug!("Importing file '{}'", path.display());

    // Read the lines for the file
    let mut lines = handle.lines();
    while let Some(line) = lines.next_line().await.transpose() {
        // Unwrap the line
        let line: String = line.map_err(|source| Error::FileRead { path: path.into(), source })?;

        // See if a file is included
        match potentially_include_async(imported, path, &line).await? {
            Some(Some((child_path, child_handle))) => {
                load_input_async(imported, &child_path, TBufReader::new(child_handle), child).await?;
            },
            // We don't want to write the line since we already imported it
            Some(None) => {},
            None => {
                child.write_all(line.as_bytes()).await.map_err(|source| Error::ChildWrite { source })?;
                child.write_all(b"\n").await.map_err(|source| Error::ChildWrite { source })?;
            },
        }
    }

    // Done!
    Ok(())
}





/***** LIBRARY *****/
/// Compiles a (tree of) `.eflint` files using Olaf's `eflint-to-json` compiler.
///
/// Resolves relative paths in the files as relative to the file in which they occur.
///
/// # Arguments
/// - `input`: The input file to compile. Any `#include`s and `#require`s will be handled, building a tree of files to import.
/// - `output`: Some writer to compile to.
/// - `compiler`: If given, will not download a compiler to `/tmp/eflint-to-json` but will instead use the given one.
///
/// # Errors
/// This function may error for a plethora of reasons.
pub fn compile(input_path: &Path, mut output: impl Write, compiler_path: Option<&Path>) -> Result<(), Error> {
    info!("Compiling input at '{}'", input_path.display());

    // Resolve the compiler
    let compiler_path: Cow<Path> = match compiler_path {
        Some(path) => Cow::Borrowed(path),
        None => {
            // Get the output path
            let compiler_path: PathBuf = std::env::temp_dir().join("eflint-to-json");

            // Download it if it does not exist (or at least, give it a try)
            if !compiler_path.exists() {
                // Download the file...
                download_file(
                    COMPILER_URL,
                    &compiler_path,
                    DownloadSecurity { checksum: Some(&COMPILER_CHECKSUM), https: true },
                    Some(Style::new().bold().green()),
                )
                .map_err(|source| Error::CompilerDownload {
                    from:   COMPILER_URL.into(),
                    to:     compiler_path.clone(),
                    source: Box::new(source),
                })?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt as _;

                    // ...and make it executable
                    let mut perms: Permissions =
                        fs::metadata(&compiler_path).map_err(|source| Error::FileMetadata { path: compiler_path.clone(), source })?.permissions();

                    perms.set_mode(perms.mode() | 0o500);
                    fs::set_permissions(&compiler_path, perms).map_err(|source| Error::FilePermissions { path: compiler_path.clone(), source })?;
                }
            }

            // Return the path
            Cow::Owned(compiler_path)
        },
    };
    debug!("Using compiler at: '{}'", compiler_path.display());

    // Open the input file
    debug!("Opening input file '{}'", input_path.display());
    let input = File::open(input_path).map_err(|source| Error::FileOpen { path: input_path.into(), source })?;

    // Alrighty well open a handle to the compiler
    debug!("Spawning compiler '{}'", compiler_path.display());
    let mut cmd: Command = Command::new(compiler_path.to_string_lossy().as_ref());
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut handle: Child = cmd.spawn().map_err(|source| Error::Spawn { cmd: format!("{cmd:?}"), source })?;

    // Feed the input to the compiler, analyzing for `#input(...)` and `#require(...)`
    debug!("Reading input to child process...");
    let mut stdin: ChildStdin = handle.stdin.take().unwrap();
    let mut included: HashSet<PathBuf> = HashSet::new();
    load_input(&mut included, input_path, BufReader::new(input), &mut stdin)?;
    drop(stdin);

    // Wait until the process is finished
    debug!("Waiting for child process to complete...");
    let status: ExitStatus = handle.wait().map_err(|source| Error::ChildWait { source })?;

    if !status.success() {
        return Err(Error::ChildFailed {
            cmd: format!("{cmd:?}"),
            status,
            output: ChildStreams(vec![
                ChildStream::new("stdout", handle.stdout.take().unwrap()),
                ChildStream::new("stderr", handle.stderr.take().unwrap()),
            ]),
        });
    }

    // Alrighty, now it's time to stream the output of the child to the output file
    debug!("Writing child process output to given output...");
    let mut chunk: [u8; 65535] = [0; 65535];
    let mut stdout: ChildStdout = handle.stdout.take().unwrap();
    loop {
        // Read the next chunk
        let chunk_len: usize = stdout.read(&mut chunk).map_err(|source| Error::ChildRead { source })?;

        if chunk_len == 0 {
            break;
        }

        // Write to the file
        output.write_all(&chunk[..chunk_len]).map_err(|source| Error::WriterWrite { source })?;
    }

    // Done
    Ok(())
}

/// Compiles a (tree of) `.eflint` files using Olaf's `eflint-to-json` compiler.
///
/// Resolves relative paths in the files as relative to the file in which they occur.
///
/// # Arguments
/// - `input`: The input file to compile. Any `#include`s and `#require`s will be handled, building a tree of files to import.
/// - `output`: Some writer to compile to.
/// - `compiler`: If given, will not download a compiler to `/tmp/eflint-to-json` but will instead use the given one.
///
/// # Errors
/// This function may error for a plethora of reasons.
#[cfg(feature = "async-tokio")]
pub async fn compile_async(input_path: &Path, mut output: impl Write, compiler_path: Option<&Path>) -> Result<(), Error> {
    info!("Compiling input at '{}'", input_path.display());

    // Resolve the compiler
    let compiler_path: Cow<Path> = match compiler_path {
        Some(path) => Cow::Borrowed(path),
        None => {
            // Get the output path
            let compiler_path: PathBuf = std::env::temp_dir().join("eflint-to-json");

            // Download it if it does not exist (or at least, give it a try)
            if !compiler_path.exists() {
                // Download the file...
                download_file_async(
                    COMPILER_URL,
                    &compiler_path,
                    DownloadSecurity { checksum: Some(&COMPILER_CHECKSUM), https: true },
                    Some(Style::new().bold().green()),
                )
                .await
                .map_err(|source| Error::CompilerDownload {
                    from:   COMPILER_URL.into(),
                    to:     compiler_path.clone(),
                    source: Box::new(source),
                })?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt as _;

                    // ...and make it executable
                    debug!("Making compiler '{}' executable...", compiler_path.display());
                    let mut perms: Permissions = tfs::metadata(&compiler_path)
                        .await
                        .map_err(|source| Error::FileMetadata { path: compiler_path.clone(), source })?
                        .permissions();
                    perms.set_mode(perms.mode() | 0o500);

                    tfs::set_permissions(&compiler_path, perms)
                        .await
                        .map_err(|source| Error::FilePermissions { path: compiler_path.clone(), source })?;
                }
            }

            // Return the path
            Cow::Owned(compiler_path)
        },
    };
    debug!("Using compiler at: '{}'", compiler_path.display());

    // Open the input file
    debug!("Opening input file '{}'", input_path.display());
    let input = TFile::open(input_path).await.map_err(|source| Error::FileOpen { path: input_path.into(), source })?;

    // Alrighty well open a handle to the compiler
    debug!("Spawning compiler '{}'", compiler_path.display());
    let mut cmd: TCommand = TCommand::new(compiler_path.to_string_lossy().as_ref());
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut handle = cmd.spawn().map_err(|source| Error::Spawn { cmd: format!("{cmd:?}"), source })?;

    // Feed the input to the compiler, analyzing for `#input(...)` and `#require(...)`
    debug!("Reading input to child process...");
    let mut stdin: TChildStdin = handle.stdin.take().unwrap();
    let mut included: HashSet<PathBuf> = HashSet::new();
    load_input_async(&mut included, input_path, TBufReader::new(input), &mut stdin).await?;
    drop(stdin);

    // Wait until the process is finished
    debug!("Waiting for child process to complete...");
    let status = handle.wait().await.map_err(|source| Error::ChildWait { source })?;

    if !status.success() {
        return Err(Error::ChildFailed {
            cmd: format!("{cmd:?}"),
            status,
            output: ChildStreams(vec![
                ChildStream::new_async("stdout", handle.stdout.take().unwrap()).await,
                ChildStream::new_async("stderr", handle.stderr.take().unwrap()).await,
            ]),
        });
    }

    // Alrighty, now it's time to stream the output of the child to the output file
    debug!("Writing child process output to given output...");
    let mut chunk = [0; 65535];
    let mut stdout: TChildStdout = handle.stdout.take().unwrap();
    loop {
        // Read the next chunk
        let chunk_len: usize = stdout.read(&mut chunk).await.map_err(|source| Error::ChildRead { source })?;

        if chunk_len == 0 {
            break;
        }

        // Write to the file
        output.write_all(&chunk[..chunk_len]).map_err(|source| Error::WriterWrite { source })?;
    }

    // Done
    Ok(())
}
