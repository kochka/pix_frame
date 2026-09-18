//! Minimal SNTP client: one request, one reply, the transmit timestamp.
//!
//! Enough to set the clock within a second, which is all a wall clock on a
//! 16×16 matrix needs.

use embassy_net::dns::DnsQueryType;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{IpAddress, Stack};
use embassy_time::{Duration, with_timeout};

const NTP_PORT: u16 = 123;
const PACKET_LEN: usize = 48;
/// Seconds between the NTP epoch (1900) and the Unix epoch (1970).
const NTP_TO_UNIX: u64 = 2_208_988_800;
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, defmt::Format)]
pub enum SntpError {
    Dns,
    NoAddress,
    Bind,
    Send,
    Timeout,
    Receive,
    /// The reply was too short, not from a server, or a kiss-of-death.
    InvalidReply,
}

/// Asks `server` for the current time; returns seconds since 1970-01-01 UTC.
pub async fn query(stack: Stack<'_>, server: &str) -> Result<u64, SntpError> {
    let addresses = stack
        .dns_query(server, DnsQueryType::A)
        .await
        .map_err(|_| SntpError::Dns)?;
    let address: IpAddress = *addresses.first().ok_or(SntpError::NoAddress)?;

    let mut rx_meta = [PacketMetadata::EMPTY; 2];
    let mut rx_buffer = [0u8; 128];
    let mut tx_meta = [PacketMetadata::EMPTY; 2];
    let mut tx_buffer = [0u8; 128];
    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    socket.bind(0).map_err(|_| SntpError::Bind)?;

    let mut request = [0u8; PACKET_LEN];
    // Leap indicator 0, version 4, mode 3 (client).
    request[0] = 0x23;
    socket
        .send_to(&request, (address, NTP_PORT))
        .await
        .map_err(|_| SntpError::Send)?;

    let mut reply = [0u8; PACKET_LEN];
    let (len, _) = with_timeout(REPLY_TIMEOUT, socket.recv_from(&mut reply))
        .await
        .map_err(|_| SntpError::Timeout)?
        .map_err(|_| SntpError::Receive)?;
    parse_reply(&reply[..len])
}

/// Extracts the Unix time from a server reply.
pub fn parse_reply(reply: &[u8]) -> Result<u64, SntpError> {
    if reply.len() < PACKET_LEN {
        return Err(SntpError::InvalidReply);
    }
    let mode = reply[0] & 0b111;
    let stratum = reply[1];
    if mode != 4 || stratum == 0 {
        return Err(SntpError::InvalidReply);
    }
    let transmit = u64::from(u32::from_be_bytes([
        reply[40], reply[41], reply[42], reply[43],
    ]));
    if transmit == 0 {
        return Err(SntpError::InvalidReply);
    }
    // NTP seconds wrap in 2036; values below the Unix epoch belong to era 1.
    let seconds = if transmit >= NTP_TO_UNIX {
        transmit - NTP_TO_UNIX
    } else {
        transmit + (1u64 << 32) - NTP_TO_UNIX
    };
    Ok(seconds)
}
