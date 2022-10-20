use axum::async_trait;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use nfs;
use nix::{fcntl::OFlag, sys::stat::Mode};
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
    NFS(nfs::Error),
    Url(url::ParseError),
    Http(reqwest::Error),
}

impl Error {
    pub(crate) fn is_not_found(&self) -> bool {
        match self {
            Error::IO(x) if x.kind() == ErrorKind::NotFound => true,
            Error::NFS(x) if x.into_io().kind() == ErrorKind::NotFound => true,
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
            Error::NFS(x) => x.into_io(),
            Error::Url(x) => std::io::Error::new(ErrorKind::Other, x),
            Error::Http(x) => std::io::Error::new(ErrorKind::Other, x),
        }
    }
}

impl From<nfs::Error> for Error {
    fn from(err: nfs::Error) -> Self {
        Error::NFS(err)
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

#[async_trait]
pub(crate) trait Library {
    async fn get_song(
        &self,
        uri: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>>;
}

pub(crate) async fn get_library(path: &str) -> Result<Box<dyn Library + Send + Sync>> {
    let lib: Box<dyn Library + Send + Sync> =
        if path.starts_with("http://") || path.starts_with("https://") {
            Box::new(HTTPLibrary::new(Url::parse(path)?))
        } else if path.starts_with("nfs://") {
            Box::new(NFSLibrary::new(Url::parse(path)?).await?)
        } else {
            Box::new(FSLibrary::new(Path::new(path))?)
        };

    Ok(lib)
}

struct FSLibrary {
    root: PathBuf,
}

// FSLibrary implements Library on top of an ordinary file system.
impl FSLibrary {
    fn new(root: &Path) -> Result<Self> {
        Ok(FSLibrary {
            root: root.to_path_buf(),
        })
    }
}

#[async_trait]
impl Library for FSLibrary {
    async fn get_song(
        &self,
        uri: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>> {
        let uri = uri.to_string();
        let file = File::open(self.root.join(Path::new(&uri))).await?;

        Ok(ReaderStream::new(file)
            .map(|x| x.map_err(Into::into))
            .boxed())
    }
}

struct HTTPLibrary {
    base: Url,
}

// HTTPLibrary implements Library on top of HTTP/HTTPS server.
impl HTTPLibrary {
    fn new(base: Url) -> Self {
        HTTPLibrary { base: base.clone() }
    }
}

#[async_trait]
impl Library for HTTPLibrary {
    async fn get_song(
        &self,
        uri: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>> {
        let stream = reqwest::get(self.base.join(uri)?).await?.bytes_stream();

        Ok(stream.map(|x| x.map_err(Into::into)).boxed())
    }
}

struct NFSLibrary {
    client: nfs::Client,
}

// NFSLibrary implements Library on top of an NFS share.
impl NFSLibrary {
    async fn new(base: Url) -> Result<Self> {
        Ok(NFSLibrary {
            client: nfs::Client::mount(base).await?,
        })
    }
}

#[async_trait]
impl Library for NFSLibrary {
    async fn get_song(
        &self,
        uri: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>> {
        let uri = uri.to_string();
        let file = self
            .client
            .open(
                Path::new(&uri),
                OFlag::O_RDONLY,
                Mode::from_bits_truncate(0o644),
            )
            .await?;

        Ok(ReaderStream::new(file)
            .map(|x| x.map_err(Into::into))
            .boxed())
    }
}
