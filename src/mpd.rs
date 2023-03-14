use axum::async_trait;
use mpd_client::{
    client::{CommandError, ConnectWithPasswordError},
    commands::{Ping, SetBinaryLimit},
    Client,
};
use std::net::SocketAddr;
use tokio::net::TcpStream;

#[derive(Clone)]
pub struct ConnectionManager {
    address: SocketAddr,
    password: Option<String>,
}

impl ConnectionManager {
    pub fn new(address: &SocketAddr, password: &Option<String>) -> ConnectionManager {
        ConnectionManager {
            address: *address,
            password: password.clone(),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Connect(std::io::Error),
    ConnectWithPassword(ConnectWithPasswordError),
    Command(CommandError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Connect(err) => write!(f, "{err}"),
            Error::ConnectWithPassword(err) => write!(f, "{err}"),
            Error::Command(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for Error {}

#[async_trait]
impl bb8::ManageConnection for ConnectionManager {
    type Connection = Client;
    type Error = Error;
    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let connection = TcpStream::connect(self.address)
            .await
            .map_err(Error::Connect)?;
        let (client, _) = Client::connect_with_password_opt(connection, self.password.as_deref())
            .await
            .map_err(Error::ConnectWithPassword)?;

        Ok(client)
    }

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.command(Ping).await.map_err(Error::Command)
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        conn.is_connection_closed()
    }
}

#[derive(Debug)]
pub struct ConnectionCustomizer;

#[async_trait]
impl bb8::CustomizeConnection<Client, Error> for ConnectionCustomizer {
    async fn on_acquire(&self, conn: &mut Client) -> Result<(), Error> {
        conn.command(SetBinaryLimit(128 * 1024))
            .await
            .map_err(Error::Command)
    }
}

pub(crate) mod commands {}
