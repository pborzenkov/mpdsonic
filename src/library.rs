use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::{
    error::Error as StdError,
    fmt,
    io::ErrorKind,
    path::{Path, PathBuf},
    pin::Pin,
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

#[derive(Debug)]
pub(crate) enum Error {
    IO(std::io::Error),
}

impl Error {
    pub fn is_not_found(&self) -> bool {
        match self {
            Error::IO(x) if x.kind() == ErrorKind::NotFound => true,
            _ => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl StdError for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IO(err)
    }
}

impl Into<std::io::Error> for Error {
    fn into(self) -> std::io::Error {
        match self {
            Error::IO(x) => x,
        }
    }
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

pub enum Library {
    FS(FSLibrary),
}

impl Library {
    pub(crate) fn new(path: &str) -> Self {
        Library::FS(FSLibrary::new(Path::new(path)))
    }

    pub(crate) async fn get_song(
        &self,
        uri: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>> {
        match self {
            Library::FS(lib) => lib
                .get_song(uri)
                .await
                .map(|s| s.map(|x| x.map_err(Into::into)).boxed()),
        }
    }
}

pub struct FSLibrary {
    root: PathBuf,
}

// FSLibrary implements Library on top of an ordinary file system.
impl FSLibrary {
    fn new(root: &Path) -> Self {
        FSLibrary {
            root: root.to_path_buf(),
        }
    }

    async fn get_song(&self, uri: &str) -> Result<ReaderStream<File>> {
        let uri = uri.to_string();
        let file = File::open(self.root.join(Path::new(&uri))).await?;

        Ok(ReaderStream::new(file))
    }
}
