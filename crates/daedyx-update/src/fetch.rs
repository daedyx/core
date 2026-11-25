//! Abruf ohne eigenes Netz („sans-io“): Die App liefert einen `Transport` (echte Umsetzung mit ureq/rustls in U2,
//! in Tests ein gefälschter Server). Hier liegen die Regeln, die für jeden Transport gelten: nur `https`, nur
//! erlaubte Rechner (auch nach Weiterleitungen), höchstens 3 Weiterleitungen, Größengrenze **während** des Ladens
//! (Abbruch beim ersten Byte zu viel), angekündigte Länge muss stimmen.
use std::io::{ErrorKind, Read, Write};

/// Rechner, die angefragt werden dürfen: GitHub selbst und die Download-Rechner, auf die es weiterleitet.
pub const ALLOWED_HOSTS: [&str; 3] =
    ["github.com", "objects.githubusercontent.com", "release-assets.githubusercontent.com"];
/// Höchstzahl der Weiterleitungen je Abruf.
pub const MAX_REDIRECTS: usize = 3;
const MAX_URL_BYTES: usize = 8192;

/// Eine geprüfte `https`-Adresse ohne Anmeldedaten, Port und Fragment, Rechnername klein geschrieben.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpsUrl {
    text: String,
    host_len: usize,
}

impl HttpsUrl {
    pub fn parse(s: &str) -> Result<HttpsUrl, FetchError> {
        let rest = s.strip_prefix("https://").ok_or(FetchError::BadAddress)?;
        let (host, path) = rest.split_at(rest.find('/').ok_or(FetchError::BadAddress)?);
        let host_ok = !host.is_empty()
            && host.len() <= 253
            && !host.starts_with(['.', '-'])
            && !host.ends_with(['.', '-'])
            && host.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-'));
        let path_ok =
            s.len() <= MAX_URL_BYTES && path.bytes().all(|b| (0x21..=0x7e).contains(&b) && b != b'\\' && b != b'#');
        if host_ok && path_ok {
            Ok(HttpsUrl { text: s.to_owned(), host_len: host.len() })
        } else {
            Err(FetchError::BadAddress)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn host(&self) -> &str {
        self.text.get(8..8usize.saturating_add(self.host_len)).unwrap_or_default()
    }
}

/// Antwort eines Transports auf **eine** Anfrage. Der Transport folgt Weiterleitungen nie selbst.
pub enum Reply<'a> {
    /// 2xx mit Inhalt; `length` aus dem Kopf, falls angegeben.
    Body { length: Option<u64>, reader: Box<dyn Read + 'a> },
    /// 3xx mit dem Inhalt des `Location`-Kopfes.
    Redirect(String),
    /// Jeder andere Status.
    Status(u16),
}

/// Fehler des Transports beim Verbinden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    /// Kein Netz, Name nicht auflösbar, Verbindung abgelehnt.
    Offline,
    /// Zeitgrenze erreicht.
    Timeout,
    /// TLS oder anderer Fehler.
    Failed,
}

/// Netz-Schnittstelle, die der Aufrufer liefert.
pub trait Transport {
    fn get(&mut self, url: &HttpsUrl) -> Result<Reply<'_>, TransportError>;
}

/// Warum ein Abruf scheiterte (englisch, nur Diagnose, ohne Adressen und Inhalte).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FetchError {
    #[error("no connection")]
    Offline,
    #[error("timed out")]
    Timeout,
    #[error("transfer interrupted")]
    Interrupted,
    #[error("server answered with status {0}")]
    Status(u16),
    #[error("address is not a plain https address")]
    BadAddress,
    #[error("address is on a host that is not allowed")]
    ForeignHost,
    #[error("too many redirects")]
    TooManyRedirects,
    #[error("response is larger than allowed")]
    TooLarge,
    #[error("writing failed: {0:?}")]
    Write(ErrorKind),
}

fn from_transport(e: TransportError) -> FetchError {
    match e {
        TransportError::Offline => FetchError::Offline,
        TransportError::Timeout => FetchError::Timeout,
        TransportError::Failed => FetchError::Interrupted,
    }
}

fn copy_limited(
    reader: &mut dyn Read,
    limit: u64,
    sink: &mut dyn Write,
    progress: &mut dyn FnMut(u64),
) -> Result<u64, FetchError> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => return Ok(total),
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) if matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {
                return Err(FetchError::Timeout)
            }
            Err(_) => return Err(FetchError::Interrupted),
        };
        total = total.saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
        if total > limit {
            return Err(FetchError::TooLarge);
        }
        sink.write_all(buf.get(..n).unwrap_or_default()).map_err(|e| FetchError::Write(e.kind()))?;
        progress(total);
    }
}

/// Holt `url` höchstens `limit` Bytes groß nach `sink`; `progress` bekommt die bisher geladenen Bytes.
pub fn get_to(
    transport: &mut dyn Transport,
    url: &str,
    limit: u64,
    sink: &mut dyn Write,
    progress: &mut dyn FnMut(u64),
) -> Result<u64, FetchError> {
    let mut current = HttpsUrl::parse(url)?;
    for hop in 0..=MAX_REDIRECTS {
        if !ALLOWED_HOSTS.contains(&current.host()) {
            return Err(FetchError::ForeignHost);
        }
        match transport.get(&current).map_err(from_transport)? {
            Reply::Redirect(_) if hop == MAX_REDIRECTS => return Err(FetchError::TooManyRedirects),
            Reply::Redirect(next) => current = HttpsUrl::parse(&next)?,
            Reply::Status(code) => return Err(FetchError::Status(code)),
            Reply::Body { length, mut reader } => {
                if length.is_some_and(|l| l > limit) {
                    return Err(FetchError::TooLarge);
                }
                let got = copy_limited(&mut reader, limit, sink, progress)?;
                return match length {
                    Some(l) if l != got => Err(FetchError::Interrupted),
                    _ => Ok(got),
                };
            }
        }
    }
    Err(FetchError::TooManyRedirects)
}

/// Holt eine kleine Datei (`entry.json`, Bündel) als Bytes.
pub fn get_bytes(transport: &mut dyn Transport, url: &str, limit: u64) -> Result<Vec<u8>, FetchError> {
    let mut out = Vec::new();
    get_to(transport, url, limit, &mut out, &mut |_| {})?;
    Ok(out)
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
