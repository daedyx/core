//! Gefälschter Server ohne Netz für die Tests: jede Adresse hat eine feste Antwort; gezählt wird jede Anfrage.
use crate::fetch::{HttpsUrl, Reply, Transport, TransportError};
use std::collections::BTreeMap;
use std::io::{self, Read};

#[derive(Clone)]
pub enum Fake {
    /// Inhalt mit angekündigter Länge (oder ohne Kopf).
    Body(Vec<u8>, Option<u64>),
    /// Inhalt, danach ein Lesefehler dieser Art (Abbruch mitten im Laden).
    BodyThenError(Vec<u8>, io::ErrorKind),
    Redirect(String),
    Status(u16),
    Fail(TransportError),
}

impl Fake {
    pub fn ok(bytes: &[u8]) -> Fake {
        Fake::Body(bytes.to_vec(), Some(bytes.len() as u64))
    }
}

#[derive(Default)]
pub struct FakeServer {
    pub routes: BTreeMap<String, Fake>,
    pub requests: Vec<String>,
}

impl FakeServer {
    pub fn with(mut self, url: &str, reply: Fake) -> FakeServer {
        self.routes.insert(url.to_owned(), reply);
        self
    }
}

struct FailingTail {
    data: io::Cursor<Vec<u8>>,
    error: io::ErrorKind,
}

impl Read for FailingTail {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.data.read(buf)? {
            0 => Err(io::Error::from(self.error)),
            n => Ok(n),
        }
    }
}

impl Transport for FakeServer {
    fn get(&mut self, url: &HttpsUrl) -> Result<Reply<'_>, TransportError> {
        self.requests.push(url.as_str().to_owned());
        match self.routes.get(url.as_str()).cloned().unwrap_or(Fake::Status(404)) {
            Fake::Body(b, length) => Ok(Reply::Body { length, reader: Box::new(io::Cursor::new(b)) }),
            Fake::BodyThenError(b, error) => {
                Ok(Reply::Body { length: None, reader: Box::new(FailingTail { data: io::Cursor::new(b), error }) })
            }
            Fake::Redirect(to) => Ok(Reply::Redirect(to)),
            Fake::Status(c) => Ok(Reply::Status(c)),
            Fake::Fail(e) => Err(e),
        }
    }
}
