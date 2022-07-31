use bytes::Bytes;
use futures::{Stream, StreamExt};
use reqwest::StatusCode;
use std::{
    error::Error as StdError,
    fmt,
    io::ErrorKind,
    path::{Path, PathBuf},
    pin::Pin,
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use url::Url;

#[derive(Debug)]
pub(crate) enum Error {
    IO(std::io::Error),
    Url(url::ParseError),
    Http(reqwest::Error),
}

impl Error {
    pub fn is_not_found(&self) -> bool {
        match self {
            Error::IO(x) if x.kind() == ErrorKind::NotFound => true,
            Error::Http(x) if x.status().map_or(false, |v| v == StatusCode::NOT_FOUND) => true,
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

impl From<Error> for std::io::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::IO(x) => x,
            Error::Url(x) => std::io::Error::new(ErrorKind::Other, x),
            Error::Http(x) => std::io::Error::new(ErrorKind::Other, x),
        }
    }
}

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Self {
        Error::Url(err)
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Http(err)
    }
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

pub enum Library {
    FS(FSLibrary),
    Http(HTTPLibrary),
}

impl Library {
    pub(crate) fn new(path: &str) -> Result<Self> {
        let lib = if path.starts_with("http://") || path.starts_with("https://") {
            Library::Http(HTTPLibrary::new(&Url::parse(path)?))
        } else {
            Library::FS(FSLibrary::new(Path::new(path)))
        };

        Ok(lib)
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
            Library::Http(lib) => lib
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

pub struct HTTPLibrary {
    base: Url,
}

// HTTPLibrary implements Library on top of HTTP/HTTPS server.
impl HTTPLibrary {
    fn new(base: &Url) -> Self {
        HTTPLibrary { base: base.clone() }
    }

    async fn get_song(&self, uri: &str) -> Result<impl Stream<Item = reqwest::Result<Bytes>>> {
        let stream = reqwest::get(self.base.join(uri)?).await?.bytes_stream();

        Ok(stream)
    }
}
