//  DOWNLOAD.rs
//    by Lut99
//
//  Created:
//    29 Nov 2023, 15:11:58
//  Last edited:
//    12 Jun 2024, 17:45:43
//  Auto updated?
//    Yes
//
//  Description:
//!   File to download stuff from the World Wide Web using [`reqwest`].
//

use std::fmt::{Display, Formatter, Result as FResult};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::{error, fs};

use console::Style;
use futures_util::StreamExt as _;
use indicatif::{ProgressBar, ProgressStyle};
use log::debug;
use reqwest::{Client, Request, Response, StatusCode, Url, blocking};
use sha2::{Digest as _, Sha256};
use tokio::fs as tfs;
use tokio::io::AsyncWriteExt as _;

/***** ERRORS *****/
/// Wraps the contents of an error body.
#[derive(Debug)]
pub struct ResponseBodyError(pub String);
impl Display for ResponseBodyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "{}", self.0) }
}
impl error::Error for ResponseBodyError {}

/// Defines errors occurring with [`download_file_async()`].
#[derive(Debug)]
pub enum Error {
    /// Failed to create a file.
    FileCreate { path: PathBuf, err: std::io::Error },
    /// Failed to write to the output file.
    FileWrite { path: PathBuf, err: std::io::Error },
    /// The checksum of a file was not what we expected.
    FileChecksum { path: PathBuf, got: String, expected: String },

    /// Directory not found.
    DirNotFound { path: PathBuf },

    /// The given address did not have HTTPS enabled.
    NotHttps { address: String },
    /// Failed to send a request to the given address.
    Request { address: String, err: reqwest::Error },
    /// The given server responded with a non-2xx status code.
    RequestFailure { address: String, code: StatusCode, err: Option<ResponseBodyError> },
    /// Failed to download the full file stream.
    Download { address: String, err: reqwest::Error },
}
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use Error::*;
        match self {
            FileCreate { path, .. } => write!(f, "Failed to create output file '{}'", path.display()),
            FileWrite { path, .. } => write!(f, "Failed to write to output file '{}'", path.display()),
            FileChecksum { path, got, expected } => {
                write!(f, "Checksum of downloaded file '{}' is incorrect: expected '{}', got '{}'", path.display(), got, expected)
            },

            DirNotFound { path } => write!(f, "Directory '{}' not found", path.display()),

            NotHttps { address } => {
                write!(f, "Security policy requires HTTPS is enabled, but '{address}' does not enable it (or we cannot parse the URL)")
            },
            Request { address, .. } => write!(f, "Failed to send GET-request to '{address}'"),
            RequestFailure { address, code, .. } => {
                write!(f, "GET-request to '{}' failed with status code {} ({})", address, code.as_u16(), code.canonical_reason().unwrap_or("???"))
            },
            Download { address, .. } => write!(f, "Failed to download file '{address}'"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use Error::*;
        match self {
            FileCreate { err, .. } => Some(err),
            FileWrite { err, .. } => Some(err),
            FileChecksum { .. } => None,

            DirNotFound { .. } => None,

            NotHttps { .. } => None,
            Request { err, .. } => Some(err),
            RequestFailure { err, .. } => err.as_ref().map(|err| {
                // Use a little bit of indirection to coerce into the trait object
                let err: &dyn error::Error = err;
                err
            }),
            Download { err, .. } => Some(err),
        }
    }
}

/***** AUXILLARY *****/
/// Defines things to do to assert a downloaded file is secure and what we expect.
#[derive(Clone, Debug)]
pub struct DownloadSecurity<'c> {
    /// If not `None`, then it defined the checksum that the file should have.
    pub checksum: Option<&'c [u8]>,
    /// If true, then the file can only be downloaded over HTTPS.
    pub https:    bool,
}
impl<'c> DownloadSecurity<'c> {
    /// Constructor for the DownloadSecurity that enables with all security measures enabled.
    ///
    /// This will provide you with the most security, but is also the slowest method (since it does both encryption and checksum computation).
    ///
    /// Usually, it sufficies to only use a checksum (`DownloadSecurity::checksum()`) if you know what the file looks like a-priori.
    ///
    /// # Arguments
    /// - `checksum`: The checksum that we want the file to have. If you are unsure, give a garbage checksum, then run the function once and check what the file had (after making sure the download went correctly, of course).
    ///
    /// # Returns
    /// A new DownloadSecurity instance that will make your downloaded file so secure you can use it to store a country's defecit (not legal advice).
    #[inline]
    pub fn all(checkum: &'c [u8]) -> Self { Self { checksum: Some(checkum), https: true } }

    /// Constructor for the DownloadSecurity that enables checksum verification only.
    ///
    /// Using this method is considered secure, since it guarantees that the downloaded file is what we expect. It is thus safe to use if you don't trust either the network or the remote praty.
    ///
    /// Note, however, that this method only works if you know a-priori what the downloaded file should look like. If not, you must use another security method (e.g., `DownloadSecurity::https()`).
    ///
    /// # Arguments
    /// - `checksum`: The checksum that we want the file to have. If you are unsure, give a garbage checksum, then run the function once and check what the file had (after making sure the download went correctly, of course).
    ///
    /// # Returns
    /// A new DownloadSecurity instance that will make sure your file has the given checksum before returning.
    #[inline]
    pub fn checksum(checkum: &'c [u8]) -> Self { Self { checksum: Some(checkum), https: false } }

    /// Constructor for the DownloadSecurity that forces downloads to go over HTTPS.
    ///
    /// You should only use this method if you trust the remote party. However, if you do, then it guarantees that there was no man-in-the-middle changing the downloaded file.
    ///
    /// # Returns
    /// A new DownloadSecurity instance that will make sure your file if downloaded over HTTPS only.
    #[inline]
    pub fn https() -> Self { Self { checksum: None, https: true } }

    /// Constructor for the DownloadSecurity that disabled all security measures.
    ///
    /// For obvious reasons, this security is not recommended unless you trust both the network _and_ the remote party.
    ///
    /// # Returns
    /// A new DownloadSecurity instance that will require no additional security measures on the downloaded file.
    #[inline]
    pub fn none() -> Self { Self { checksum: None, https: false } }
}
impl Display for DownloadSecurity<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        // Write what is enabled
        if let Some(checksum) = &self.checksum {
            write!(f, "Checksum ({})", hex::encode(checksum))?;
            if self.https {
                write!(f, ", HTTPS")?;
            }
            Ok(())
        } else if self.https {
            write!(f, "HTTPS")
        } else {
            write!(f, "None")
        }
    }
}

/***** LIBRARY *****/
/// Downloads some file from the interwebs to the given location.
///
/// Courtesy of the Brane project (<https://github.com/braneframework/brane/blob/master/brane-shr/src/fs.rs#L1285C1-L1463C2>).
///
/// # Arguments
/// - `source`: The URL to download the file from.
/// - `target`: The location to download the file to.
/// - `verification`: Some method to verify the file is what we think it is. See the `VerifyMethod`-enum for more information.
/// - `verbose`: If not `None`, will print to the output with accents given in the given `Style` (use a non-exciting Style to print without styles).
///
/// # Returns
/// Nothing, except that when it does you can assume a file exists at the given location.
///
/// # Errors
/// This function may error if we failed to download the file or write it (which may happen if the parent directory of `local` does not exist, among other things).
pub fn download_file(source: impl AsRef<str>, target: impl AsRef<Path>, security: DownloadSecurity<'_>, verbose: Option<Style>) -> Result<(), Error> {
    let source: &str = source.as_ref();
    let target: &Path = target.as_ref();
    debug!("Downloading '{}' to '{}' (Security: {})...", source, target.display(), security);
    if let Some(style) = &verbose {
        println!("Downloading {}...", style.apply_to(source));
    }

    // Assert the download directory exists
    let dir: Option<&Path> = target.parent();
    if let Some(dir) = dir {
        if !dir.exists() {
            return Err(Error::DirNotFound { path: dir.into() });
        }
    }

    // Open the target file for writing
    let mut handle: fs::File = match fs::File::create(target) {
        // Ok(handle) => {
        //     // Prepare the permissions to set by reading the file's metadata
        //     let mut permissions: Permissions = match handle.metadata() {
        //         Ok(metadata) => metadata.permissions(),
        //         Err(err)     => { return Err(Error::FileMetadataError{ what: "temporary binary", path: local.into(), err }); },
        //     };
        //     permissions.set_mode(permissions.mode() | 0o100);

        //     // Set them
        //     if let Err(err) = handle.set_permissions(permissions) { return Err(Error::FilePermissionsError{ what: "temporary binary", path: local.into(), err }); }

        //     // Return the handle
        //     handle
        // },
        Ok(handle) => handle,
        Err(err) => {
            return Err(Error::FileCreate { path: target.into(), err });
        },
    };

    // Send a request
    let res: blocking::Response = if security.https {
        debug!("Sending download request to '{}' (HTTPS enabled)...", source);

        // Assert the address starts with HTTPS first
        if Url::parse(source).ok().map(|u| u.scheme() != "https").unwrap_or(true) {
            return Err(Error::NotHttps { address: source.into() });
        }

        // Send the request with a user-agent header (to make GitHub happy)
        let client: blocking::Client = blocking::Client::new();
        let req: blocking::Request = match client.get(source).header("User-Agent", "reqwest").build() {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::Request { address: source.into(), err });
            },
        };
        match client.execute(req) {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::Request { address: source.into(), err });
            },
        }
    } else {
        debug!("Sending download request to '{}'...", source);

        // Send the request with a user-agent header (to make GitHub happy)
        let client: blocking::Client = blocking::Client::new();
        let req: blocking::Request = match client.get(source).header("User-Agent", "reqwest").build() {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::Request { address: source.into(), err });
            },
        };
        match client.execute(req) {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::Request { address: source.into(), err });
            },
        }
    };

    // Assert it succeeded
    if !res.status().is_success() {
        return Err(Error::RequestFailure { address: source.into(), code: res.status(), err: res.text().ok().map(ResponseBodyError) });
    }

    // Create the progress bar based on whether if there is a length
    debug!("Downloading response to file '{}'...", target.display());
    let len: Option<u64> = res.headers().get("Content-Length").and_then(|len| len.to_str().ok()).and_then(|len| u64::from_str(len).ok());
    let prgs: Option<ProgressBar> = if verbose.is_some() {
        Some(if let Some(len) = len {
            ProgressBar::new(len)
                .with_style(ProgressStyle::with_template("    {bar:60} {bytes}/{total_bytes} {bytes_per_sec} ETA {eta_precise}").unwrap())
        } else {
            ProgressBar::new_spinner()
                .with_style(ProgressStyle::with_template("    {elapsed_precise} {bar:60} {bytes} {binary_bytes_per_sec}").unwrap())
        })
    } else {
        None
    };

    // Prepare getting a checksum if that is our method of choice
    let mut hasher: Option<Sha256> = if security.checksum.is_some() { Some(Sha256::new()) } else { None };

    // Download the response to the opened output file
    let body = match res.bytes() {
        Ok(body) => body,
        Err(err) => return Err(Error::Download { address: source.into(), err }),
    };
    for next in body.chunks(16384) {
        // Write it to the file
        if let Err(err) = handle.write(next) {
            return Err(Error::FileWrite { path: target.into(), err });
        }

        // If desired, update the hash
        if let Some(hasher) = &mut hasher {
            hasher.update(next);
        }

        // Update what we've written if needed
        if let Some(prgs) = &prgs {
            prgs.update(|state| state.set_pos(state.pos() + next.len() as u64));
        }
    }
    if let Some(prgs) = &prgs {
        prgs.finish_and_clear();
    }

    // Assert the checksums are the same if we're doing that
    if let Some(checksum) = security.checksum {
        // Finalize the hasher first
        let result = hasher.unwrap().finalize();
        debug!("Verifying checksum...");

        // Assert the checksums check out (wheezes)
        if &result[..] != checksum {
            return Err(Error::FileChecksum { path: target.into(), expected: hex::encode(checksum), got: hex::encode(&result[..]) });
        }

        // Print that the checksums are equal if asked
        if let Some(style) = verbose {
            // Create the dim styles
            let dim: Style = Style::new().dim();
            let accent: Style = style.dim();

            // Write it with those styles
            println!("{}{}{}", dim.apply_to(" > Checksum "), accent.apply_to(hex::encode(&result[..])), dim.apply_to(" OK"));
        }
    }

    // Done
    Ok(())
}

/// Downloads some file from the interwebs to the given location.
///
/// Courtesy of the Brane project (<https://github.com/braneframework/brane/blob/master/brane-shr/src/fs.rs#L1285C1-L1463C2>).
///
/// # Arguments
/// - `source`: The URL to download the file from.
/// - `target`: The location to download the file to.
/// - `verification`: Some method to verify the file is what we think it is. See the `VerifyMethod`-enum for more information.
/// - `verbose`: If not `None`, will print to the output with accents given in the given `Style` (use a non-exciting Style to print without styles).
///
/// # Returns
/// Nothing, except that when it does you can assume a file exists at the given location.
///
/// # Errors
/// This function may error if we failed to download the file or write it (which may happen if the parent directory of `local` does not exist, among other things).
pub async fn download_file_async(
    source: impl AsRef<str>,
    target: impl AsRef<Path>,
    security: DownloadSecurity<'_>,
    verbose: Option<Style>,
) -> Result<(), Error> {
    let source: &str = source.as_ref();
    let target: &Path = target.as_ref();
    debug!("Downloading '{}' to '{}' (Security: {})...", source, target.display(), security);
    if let Some(style) = &verbose {
        println!("Downloading {}...", style.apply_to(source));
    }

    // Assert the download directory exists
    let dir: Option<&Path> = target.parent();
    if let Some(dir) = dir {
        if !dir.exists() {
            return Err(Error::DirNotFound { path: dir.into() });
        }
    }

    // Open the target file for writing
    let mut handle: tfs::File = match tfs::File::create(target).await {
        // Ok(handle) => {
        //     // Prepare the permissions to set by reading the file's metadata
        //     let mut permissions: Permissions = match handle.metadata() {
        //         Ok(metadata) => metadata.permissions(),
        //         Err(err)     => { return Err(Error::FileMetadataError{ what: "temporary binary", path: local.into(), err }); },
        //     };
        //     permissions.set_mode(permissions.mode() | 0o100);

        //     // Set them
        //     if let Err(err) = handle.set_permissions(permissions) { return Err(Error::FilePermissionsError{ what: "temporary binary", path: local.into(), err }); }

        //     // Return the handle
        //     handle
        // },
        Ok(handle) => handle,
        Err(err) => {
            return Err(Error::FileCreate { path: target.into(), err });
        },
    };

    // Send a request
    let res: Response = if security.https {
        debug!("Sending download request to '{}' (HTTPS enabled)...", source);

        // Assert the address starts with HTTPS first
        if Url::parse(source).ok().map(|u| u.scheme() != "https").unwrap_or(true) {
            return Err(Error::NotHttps { address: source.into() });
        }

        // Send the request with a user-agent header (to make GitHub happy)
        let client: Client = Client::new();
        let req: Request = match client.get(source).header("User-Agent", "reqwest").build() {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::Request { address: source.into(), err });
            },
        };
        match client.execute(req).await {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::Request { address: source.into(), err });
            },
        }
    } else {
        debug!("Sending download request to '{}'...", source);

        // Send the request with a user-agent header (to make GitHub happy)
        let client: Client = Client::new();
        let req: Request = match client.get(source).header("User-Agent", "reqwest").build() {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::Request { address: source.into(), err });
            },
        };
        match client.execute(req).await {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::Request { address: source.into(), err });
            },
        }
    };

    // Assert it succeeded
    if !res.status().is_success() {
        return Err(Error::RequestFailure { address: source.into(), code: res.status(), err: res.text().await.ok().map(ResponseBodyError) });
    }

    // Create the progress bar based on whether if there is a length
    debug!("Downloading response to file '{}'...", target.display());
    let len: Option<u64> = res.headers().get("Content-Length").and_then(|len| len.to_str().ok()).and_then(|len| u64::from_str(len).ok());
    let prgs: Option<ProgressBar> = if verbose.is_some() {
        Some(if let Some(len) = len {
            ProgressBar::new(len)
                .with_style(ProgressStyle::with_template("    {bar:60} {bytes}/{total_bytes} {bytes_per_sec} ETA {eta_precise}").unwrap())
        } else {
            ProgressBar::new_spinner()
                .with_style(ProgressStyle::with_template("    {elapsed_precise} {bar:60} {bytes} {binary_bytes_per_sec}").unwrap())
        })
    } else {
        None
    };

    // Prepare getting a checksum if that is our method of choice
    let mut hasher: Option<Sha256> = if security.checksum.is_some() { Some(Sha256::new()) } else { None };

    // Download the response to the opened output file
    let mut stream = res.bytes_stream();
    while let Some(next) = stream.next().await {
        // Unwrap the result
        let next = match next {
            Ok(next) => next,
            Err(err) => {
                return Err(Error::Download { address: source.into(), err });
            },
        };

        // Write it to the file
        if let Err(err) = handle.write(&next).await {
            return Err(Error::FileWrite { path: target.into(), err });
        }

        // If desired, update the hash
        if let Some(hasher) = &mut hasher {
            hasher.update(&*next);
        }

        // Update what we've written if needed
        if let Some(prgs) = &prgs {
            prgs.update(|state| state.set_pos(state.pos() + next.len() as u64));
        }
    }
    if let Some(prgs) = &prgs {
        prgs.finish_and_clear();
    }

    // Assert the checksums are the same if we're doing that
    if let Some(checksum) = security.checksum {
        // Finalize the hasher first
        let result = hasher.unwrap().finalize();
        debug!("Verifying checksum...");

        // Assert the checksums check out (wheezes)
        if &result[..] != checksum {
            return Err(Error::FileChecksum { path: target.into(), expected: hex::encode(checksum), got: hex::encode(&result[..]) });
        }

        // Print that the checksums are equal if asked
        if let Some(style) = verbose {
            // Create the dim styles
            let dim: Style = Style::new().dim();
            let accent: Style = style.dim();

            // Write it with those styles
            println!("{}{}{}", dim.apply_to(" > Checksum "), accent.apply_to(hex::encode(&result[..])), dim.apply_to(" OK"));
        }
    }

    // Done
    Ok(())
}
