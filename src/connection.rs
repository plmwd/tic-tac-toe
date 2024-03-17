use std::net::SocketAddr;

use bytes::BytesMut;
use serde::de::DeserializeOwned;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

use crate::message::{Error, Message};

pub type ConnectionId = u32;

#[derive(Debug)]
pub struct Connection {
    stream: BufReader<TcpStream>,
    buffer: BytesMut,
    pub addr: SocketAddr,
}

impl Connection {
    pub fn new(socket: TcpStream, addr: SocketAddr) -> Self {
        Self {
            buffer: BytesMut::with_capacity(256),
            stream: BufReader::new(socket),
            addr,
        }
    }

    pub async fn recv<'a, T>(&mut self) -> anyhow::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        use ron::de::from_bytes;
        use ron::error::SpannedError;
        use ron::Error::{Eof, ExpectedDifferentLength};

        self.buffer.clear();
        loop {
            match from_bytes::<T>(&self.buffer) {
                Ok(mes) => return Ok(Some(mes)),
                Err(SpannedError { code, .. }) => match code {
                    ExpectedDifferentLength { .. } => {}
                    Eof => {}
                    e => {
                        println!("error reading message {:#?}", e);
                        self.send(Error::InvalidMessage(format!("{}", e)))
                            .await
                            .unwrap();
                        self.buffer.clear();
                    }
                },
            }

            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                if self.buffer.is_empty() {
                    return Ok(None);
                }
                anyhow::bail!("Connection reset by peer");
            }
        }
    }

    pub async fn send(&mut self, mes: impl Into<Message>) -> tokio::io::Result<()> {
        self.stream
            .write_all(
                format!("{}\n", ron::ser::to_string::<Message>(&mes.into()).unwrap()).as_bytes(),
            )
            .await?;
        self.stream.flush().await?;
        Ok(())
    }
}
