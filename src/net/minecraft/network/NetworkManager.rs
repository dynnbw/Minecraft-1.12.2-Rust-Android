use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use thiserror::Error;

use crate::net::minecraft::network::EnumConnectionState::ConnectionState;
use crate::net::minecraft::network::Packet::RawPacket;
use crate::net::minecraft::network::PacketBuffer::{read_var_i32, PacketCodec, CodecError};
use crate::net::minecraft::util::CryptManager::{NetCipher, SecretKey};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const IO_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Error)]
pub enum NetworkManagerError {
    #[error("unknown host")]
    UnknownHost,
    #[error("network operation timed out")]
    Timeout,
    #[error("connection closed by remote host")]
    Closed,
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Codec(#[from] CodecError),
}

#[derive(Debug)]
pub struct NetworkManager {
    stream: TcpStream,
    socketAddress: SocketAddr,
    connectionState: ConnectionState,
    codec: PacketCodec,
    encryptor: Option<NetCipher>,
    decryptor: Option<NetCipher>,
    channelOpen: bool,
}

impl NetworkManager {
    pub fn createNetworkManagerAndConnect(host: &str, port: u16) -> Result<Self, NetworkManagerError> {
        let socketAddress = (host, port).to_socket_addrs()
            .map_err(|_| NetworkManagerError::UnknownHost)?
            .next().ok_or(NetworkManagerError::UnknownHost)?;
        let stream = TcpStream::connect_timeout(&socketAddress, CONNECT_TIMEOUT)?;
        stream.set_nodelay(true).ok();
        stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
        stream.set_write_timeout(Some(CONNECT_TIMEOUT)).ok();
        Ok(Self {
            stream, socketAddress, connectionState: ConnectionState::Handshaking,
            codec: PacketCodec::default(), encryptor: None, decryptor: None, channelOpen: true,
        })
    }

    pub fn sendPacket(&mut self, packet: &RawPacket) -> Result<(), NetworkManagerError> {
        let mut encoded = self.codec.encode(packet)?;
        if let Some(cipher) = self.encryptor.as_mut() { cipher.apply(&mut encoded); }
        self.stream.write_all(&encoded)?;
        self.stream.flush()?;
        Ok(())
    }

    pub fn readPacket(&mut self) -> Result<RawPacket, NetworkManagerError> {
        let mut lengthBytes = Vec::with_capacity(5);
        let packetLength = loop {
            if lengthBytes.len() >= 5 { return Err(NetworkManagerError::Codec(CodecError::VarIntTooLarge)); }
            let byte = self.readNetworkByte(lengthBytes.is_empty())?;
            lengthBytes.push(byte);
            if byte & 0x80 == 0 {
                let mut view = lengthBytes.as_slice();
                break read_var_i32(&mut view)?;
            }
        };
        if packetLength < 0 { return Err(NetworkManagerError::Codec(CodecError::NegativeLength(packetLength))); }
        if packetLength as usize > 2 * 1024 * 1024 {
            return Err(NetworkManagerError::Codec(CodecError::PacketTooLarge { actual: packetLength as usize, maximum: 2 * 1024 * 1024 }));
        }
        let mut body = vec![0_u8; packetLength as usize];
        self.readNetworkExact(&mut body, false)?;
        let mut frame = lengthBytes;
        frame.extend_from_slice(&body);
        let mut view = frame.as_slice();
        Ok(self.codec.decode(&mut view)?)
    }

    pub fn enableEncryption(&mut self, secretKey: &SecretKey) {
        self.encryptor = Some(NetCipher::new(secretKey, true));
        self.decryptor = Some(NetCipher::new(secretKey, false));
    }

    pub fn setCompressionThreshold(&mut self, threshold: i32) {
        self.codec.set_compression_threshold(if threshold >= 0 { Some(threshold as usize) } else { None });
    }

    pub fn setReadTimeout(&self, timeout: Duration) -> Result<(), NetworkManagerError> {
        self.stream.set_read_timeout(Some(timeout))?;
        Ok(())
    }

    pub fn setConnectionState(&mut self, state: ConnectionState) { self.connectionState = state; }
    pub const fn getConnectionState(&self) -> ConnectionState { self.connectionState }
    pub const fn isChannelOpen(&self) -> bool { self.channelOpen }
    /// MCP `NetworkManager#isEncrypted`, used by GuiPlayerTabOverlay to
    /// decide whether the authenticated player-head branch is available.
    pub const fn isEncrypted(&self) -> bool { self.encryptor.is_some() }
    pub const fn getRemoteAddress(&self) -> SocketAddr { self.socketAddress }

    pub fn closeChannel(&mut self) {
        self.channelOpen = false;
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }

    fn readNetworkByte(&mut self, allowIdleTimeout: bool) -> Result<u8, NetworkManagerError> {
        let mut byte = [0_u8; 1];
        self.readNetworkExact(&mut byte, allowIdleTimeout)?;
        Ok(byte[0])
    }

    /// Reads exactly one already-started network segment without discarding
    /// protocol bytes when the socket timeout expires. An idle timeout is only
    /// surfaced before the first byte of a new packet; once a VarInt prefix or
    /// body has started, the same call retains its local progress until the
    /// frame is complete.
    fn readNetworkExact(&mut self, output: &mut [u8], allowIdleTimeout: bool) -> Result<(), NetworkManagerError> {
        let mut offset = 0_usize;
        while offset < output.len() {
            match self.stream.read(&mut output[offset..]) {
                Ok(0) => { self.channelOpen = false; return Err(NetworkManagerError::Closed); }
                Ok(read) => {
                    if let Some(cipher) = self.decryptor.as_mut() { cipher.apply(&mut output[offset..offset + read]); }
                    offset += read;
                }
                Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                    if offset == 0 && allowIdleTimeout {
                        return Err(NetworkManagerError::Timeout);
                    }
                }
                Err(error) if matches!(error.kind(), io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted | io::ErrorKind::BrokenPipe) => {
                    self.channelOpen = false; return Err(NetworkManagerError::Closed);
                }
                Err(error) => return Err(NetworkManagerError::Io(error)),
            }
        }
        Ok(())
    }
}

impl Drop for NetworkManager { fn drop(&mut self) { self.closeChannel(); } }
