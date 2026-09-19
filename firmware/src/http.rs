//! Minimal HTTP/1.0 client: one plain-text GET, the whole response read
//! into a caller-provided buffer until the server closes the connection.
//!
//! Enough for small JSON APIs; no TLS, no chunked encoding, no keep-alive.

use core::fmt::Write as _;

use embassy_net::dns::DnsQueryType;
use embassy_net::tcp::TcpSocket;
use embassy_net::{IpAddress, Stack};
use embassy_time::{Duration, with_timeout};
use heapless::String;

const HTTP_PORT: u16 = 80;
const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_REQUEST: usize = 512;

#[derive(Clone, Copy, Debug, defmt::Format)]
pub enum HttpError {
    Dns,
    NoAddress,
    Connect,
    Timeout,
    RequestTooLong,
    Send,
    Receive,
    /// The response has no status line or is not `200`.
    Status,
    /// The response does not fit in the buffer.
    ResponseTooLong,
}

/// Sends `GET path` to `host` and returns the response body.
pub async fn get<'b>(
    stack: Stack<'_>,
    host: &str,
    path: &str,
    buffer: &'b mut [u8],
) -> Result<&'b [u8], HttpError> {
    let mut request: String<MAX_REQUEST> = String::new();
    write!(
        request,
        "GET {path} HTTP/1.0\r\nHost: {host}\r\nUser-Agent: pix-frame\r\nConnection: close\r\n\r\n"
    )
    .map_err(|_| HttpError::RequestTooLong)?;

    let addresses = stack
        .dns_query(host, DnsQueryType::A)
        .await
        .map_err(|_| HttpError::Dns)?;
    let address: IpAddress = *addresses.first().ok_or(HttpError::NoAddress)?;

    let mut rx_buffer = [0u8; 1024];
    let mut tx_buffer = [0u8; MAX_REQUEST];
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(Some(TIMEOUT));

    with_timeout(TIMEOUT, socket.connect((address, HTTP_PORT)))
        .await
        .map_err(|_| HttpError::Timeout)?
        .map_err(|_| HttpError::Connect)?;

    let mut pending = request.as_bytes();
    while !pending.is_empty() {
        let sent = with_timeout(TIMEOUT, socket.write(pending))
            .await
            .map_err(|_| HttpError::Timeout)?
            .map_err(|_| HttpError::Send)?;
        if sent == 0 {
            return Err(HttpError::Send);
        }
        pending = &pending[sent..];
    }

    let mut len = 0;
    loop {
        if len == buffer.len() {
            return Err(HttpError::ResponseTooLong);
        }
        let read = with_timeout(TIMEOUT, socket.read(&mut buffer[len..]))
            .await
            .map_err(|_| HttpError::Timeout)?
            .map_err(|_| HttpError::Receive)?;
        if read == 0 {
            break;
        }
        len += read;
    }
    socket.close();

    body(&buffer[..len])
}

/// Splits headers from body and checks for a `200` status.
pub fn body(response: &[u8]) -> Result<&[u8], HttpError> {
    let status_ok = response.starts_with(b"HTTP/1.") && response.get(9..12) == Some(b"200");
    if !status_ok {
        return Err(HttpError::Status);
    }
    let separator = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(HttpError::Status)?;
    Ok(&response[separator + 4..])
}
