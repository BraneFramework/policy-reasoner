pub mod formatters;

use std::borrow::Cow;
use std::convert::Infallible;
use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _};
use tempfile::NamedTempFile;
use tokio::{fs as tfs, io as tio};

#[derive(Clone, Debug)]
pub enum InputFile {
    Stdin,
    File(PathBuf),
}

impl std::str::FromStr for InputFile {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "-" {
            return Ok(InputFile::Stdin);
        }

        Ok(InputFile::File(PathBuf::from(s)))
    }
}

impl InputFile {
    pub fn display(&self) -> Cow<'static, str> {
        match self {
            InputFile::Stdin => Cow::Borrowed("<stdin>"),
            InputFile::File(path_buf) => Cow::Owned(path_buf.display().to_string()),
        }
    }

    pub async fn as_file(&self) -> miette::Result<MaybeTempFile> {
        match self {
            InputFile::File(path_buf) => Ok(MaybeTempFile::File(path_buf.clone())),
            InputFile::Stdin => {
                let file = tempfile::Builder::new().suffix(".stdin").tempfile().into_diagnostic().context("Could not create temp file")?;
                let mut handle = tfs::File::create(&file)
                    .await
                    .into_diagnostic()
                    .with_context(|| format!("Failed to open temporary stdin file '{}'", file.path().display()))?;

                tio::copy(&mut tio::stdin(), &mut handle)
                    .await
                    .into_diagnostic()
                    .with_context(|| format!("Failed to write stdin to temporary file '{}'", file.path().display()))?;

                Ok(MaybeTempFile::Stdin(file))
            },
        }
    }
}

pub enum MaybeTempFile {
    File(PathBuf),
    Stdin(NamedTempFile),
}

impl std::ops::Deref for MaybeTempFile {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        match self {
            MaybeTempFile::File(path_buf) => path_buf.deref(),
            MaybeTempFile::Stdin(named_temp_file) => named_temp_file.path(),
        }
    }
}
