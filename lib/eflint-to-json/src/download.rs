//  DOWNLOAD.rs
//    by Lut99
//
//  Created:
//    29 Nov 2023, 15:11:58
//  Last edited:
//    11 Oct 2024, 16:08:06
//  Auto updated?
//    Yes
//
//  Description:
//!   File to download stuff from the World Wide Web using [`reqwest`].
//

use std::fmt::{Display, Formatter, Result as FResult};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use console::Style;
#[cfg(feature = "async-tokio")]
use futures_util::StreamExt as _;
use indicatif::{ProgressBar, ProgressStyle};
#[cfg(feature = "async-tokio")]
use reqwest::{Client, Request, Response};
use reqwest::{StatusCode, Url, blocking};
use sha2::{Digest as _, Sha256};
#[cfg(feature = "async-tokio")]
use tokio::fs as tfs;
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncWriteExt as _;

use crate::log::debug;


/***** ERRORS *****/
/// Wraps the contents of an error body.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ResponseBodyError(pub String);



/// Defines errors occurring with [`download_file()`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to create a file.
    #[error("Failed to create output file '{}'", path.display())]
    FileCreate { path: PathBuf, source: std::io::Error },
    /// Failed to write to the output file.
    #[error("Failed to write to output file '{}'", path.display())]
    FileWrite { path: PathBuf, source: std::io::Error },
    /// The checksum of a file was not what we expected.
    #[error("Checksum of downloaded file '{}' is incorrect: expected '{got}', got '{expected}'", path.display())]
    FileChecksum { path: PathBuf, got: String, expected: String },

    /// Directory not found.
    #[error("Directory '{}' not found", path.display())]
    DirNotFound { path: PathBuf },

    /// The given address did not have HTTPS enabled.
    #[error("Security policy requires HTTPS is enabled, but '{address}' does not enable it (or we cannot parse the URL)")]
    NotHttps { address: String },
    /// Failed to send a request to the given address.
    #[error("Failed to send GET-request to '{address}'")]
    Request { address: String, source: reqwest::Error },
    /// The given server responded with a non-2xx status code.
    #[error("GET-request to '{address}' failed with status code {} ({})", code.as_u16(), code.canonical_reason().unwrap_or("???"))]
    RequestFailure { address: String, code: StatusCode, source: Option<ResponseBodyError> },
    /// Failed to download the full file stream.
    #[error("Failed to download file '{address}'")]
    Download { address: String, source: reqwest::Error },
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
pub fn download_file(
    source_url: impl AsRef<str>,
    target: impl AsRef<Path>,
    security: DownloadSecurity<'_>,
    verbose: Option<Style>,
) -> Result<(), Error> {
    let source_url: &str = source_url.as_ref();
    let target: &Path = target.as_ref();
    debug!("Downloading '{}' to '{}' (Security: {})...", source_url, target.display(), security);
    if let Some(style) = &verbose {
        println!("Downloading {}...", style.apply_to(source_url));
    }

    // Assert the download directory exists
    let dir: Option<&Path> = target.parent();
    if let Some(dir) = dir {
        if !dir.exists() {
            return Err(Error::DirNotFound { path: dir.into() });
        }
    }

    // Open the target file for writing
    let mut handle = fs::File::create(target).map_err(|source| Error::FileCreate { path: target.into(), source })?;

    // Send a request
    let res: blocking::Response = if security.https {
        debug!("Sending download request to '{}' (HTTPS enabled)...", source_url);

        // Assert the address starts with HTTPS first
        if Url::parse(source_url).ok().map(|u| u.scheme() != "https").unwrap_or(true) {
            return Err(Error::NotHttps { address: source_url.into() });
        }

        // Send the request with a user-agent header (to make GitHub happy)
        let client: blocking::Client = blocking::Client::new();
        let req: blocking::Request =
            client.get(source_url).header("User-Agent", "reqwest").build().map_err(|source| Error::Request { address: source_url.into(), source })?;

        client.execute(req).map_err(|source| Error::Request { address: source_url.into(), source })?
    } else {
        debug!("Sending download request to '{}'...", source_url);

        // Send the request with a user-agent header (to make GitHub happy)
        let client: blocking::Client = blocking::Client::new();
        let req: blocking::Request =
            client.get(source_url).header("User-Agent", "reqwest").build().map_err(|source| Error::Request { address: source_url.into(), source })?;

        client.execute(req).map_err(|source| Error::Request { address: source_url.into(), source })?
    };

    // Assert it succeeded
    if !res.status().is_success() {
        return Err(Error::RequestFailure { address: source_url.into(), code: res.status(), source: res.text().ok().map(ResponseBodyError) });
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
    let body = res.bytes().map_err(|source| Error::Download { address: source_url.into(), source })?;

    for next in body.chunks(16384) {
        // Write it to the file
        handle.write(next).map_err(|source| Error::FileWrite { path: target.into(), source })?;

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
#[cfg(feature = "async-tokio")]
pub async fn download_file_async(
    source_url: impl AsRef<str>,
    target: impl AsRef<Path>,
    security: DownloadSecurity<'_>,
    verbose: Option<Style>,
) -> Result<(), Error> {
    let source_url: &str = source_url.as_ref();
    let target: &Path = target.as_ref();
    debug!("Downloading '{source_url}' to '{target}' (Security: {security})...", target = target.display());
    if let Some(style) = &verbose {
        println!("Downloading {}...", style.apply_to(source_url));
    }

    // Assert the download directory exists
    let dir: Option<&Path> = target.parent();
    if let Some(dir) = dir {
        if !dir.exists() {
            return Err(Error::DirNotFound { path: dir.into() });
        }
    }

    // Open the target file for writing
    let mut handle: tfs::File = tfs::File::create(target).await.map_err(|source| Error::FileCreate { path: target.into(), source })?;

    // Send a request
    let res: Response = if security.https {
        debug!("Sending download request to '{source_url}' (HTTPS enabled)...");

        // Assert the address starts with HTTPS first
        if Url::parse(source_url).ok().map(|u| u.scheme() != "https").unwrap_or(true) {
            return Err(Error::NotHttps { address: source_url.into() });
        }

        // Send the request with a user-agent header (to make GitHub happy)
        let client: Client = Client::new();
        let req: Request =
            client.get(source_url).header("User-Agent", "reqwest").build().map_err(|source| Error::Request { address: source_url.into(), source })?;
        client.execute(req).await.map_err(|source| Error::Request { address: source_url.into(), source })?
    } else {
        debug!("Sending download request to '{source_url}'...");

        // Send the request with a user-agent header (to make GitHub happy)
        let client: Client = Client::new();
        let req: Request =
            client.get(source_url).header("User-Agent", "reqwest").build().map_err(|source| Error::Request { address: source_url.into(), source })?;
        client.execute(req).await.map_err(|source| Error::Request { address: source_url.into(), source })?
    };

    // Assert it succeeded
    if !res.status().is_success() {
        return Err(Error::RequestFailure {
            address: source_url.into(),
            code:    res.status(),
            source:  res.text().await.ok().map(ResponseBodyError),
        });
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
        let next = next.map_err(|source| Error::Download { address: source_url.into(), source })?;

        // Write it to the file
        handle.write(&next).await.map_err(|source| Error::FileWrite { path: target.into(), source })?;

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
