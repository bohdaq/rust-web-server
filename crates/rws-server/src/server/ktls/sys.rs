//! The Linux kernel TLS interface (`include/uapi/linux/tls.h`): the
//! `TCP_ULP` switch, the per-direction crypto state, and record types on
//! `sendmsg`/`recvmsg` control messages. Declared here rather than taken from
//! `libc`, whose copies of these structs aren't exported on every target.

use std::io;
use std::os::unix::io::RawFd;

pub const SOL_TLS: libc::c_int = 282;
pub const TLS_TX: libc::c_int = 1;
pub const TLS_RX: libc::c_int = 2;
/// `cmsg_type` of the record type on `sendmsg` (set) and `recvmsg` (get).
pub const TLS_SET_RECORD_TYPE: libc::c_int = 1;
pub const TLS_GET_RECORD_TYPE: libc::c_int = 2;

pub const TLS_1_2_VERSION: u16 = 0x0303;
pub const TLS_1_3_VERSION: u16 = 0x0304;

pub const TLS_CIPHER_AES_GCM_128: u16 = 51;
pub const TLS_CIPHER_AES_GCM_256: u16 = 52;
pub const TLS_CIPHER_CHACHA20_POLY1305: u16 = 54;

/// TLS record content types (RFC 8446 §5.1).
pub const RECORD_ALERT: u8 = 21;
pub const RECORD_HANDSHAKE: u8 = 22;
pub const RECORD_APPLICATION_DATA: u8 = 23;

#[repr(C)]
#[derive(Clone, Copy)]
struct CryptoInfo {
    version: u16,
    cipher_type: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AesGcm128 {
    info: CryptoInfo,
    iv: [u8; 8],
    key: [u8; 16],
    salt: [u8; 4],
    rec_seq: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AesGcm256 {
    info: CryptoInfo,
    iv: [u8; 8],
    key: [u8; 32],
    salt: [u8; 4],
    rec_seq: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Chacha20Poly1305 {
    info: CryptoInfo,
    iv: [u8; 12],
    key: [u8; 32],
    rec_seq: [u8; 8],
}

/// One direction's kernel crypto state, ready for `setsockopt`.
pub enum CryptoState {
    AesGcm128(AesGcm128Box),
    AesGcm256(AesGcm256Box),
    Chacha20Poly1305(ChachaBox),
}

// Boxed newtypes so the key material has one home that is zeroed on drop.
pub struct AesGcm128Box(Box<AesGcm128>);
pub struct AesGcm256Box(Box<AesGcm256>);
pub struct ChachaBox(Box<Chacha20Poly1305>);

macro_rules! zero_on_drop {
    ($t:ty) => {
        impl Drop for $t {
            fn drop(&mut self) {
                let p: *mut _ = &mut *self.0;
                // SAFETY: plain-old-data struct we own; volatile so the
                // write isn't optimised away.
                unsafe { std::ptr::write_volatile(p, std::mem::zeroed()) }
            }
        }
    };
}
zero_on_drop!(AesGcm128Box);
zero_on_drop!(AesGcm256Box);
zero_on_drop!(ChachaBox);

/// The AEAD of a negotiated cipher suite, as kTLS knows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cipher {
    AesGcm128,
    AesGcm256,
    Chacha20Poly1305,
}

impl Cipher {
    pub const ALL: [Cipher; 3] = [Cipher::AesGcm128, Cipher::AesGcm256, Cipher::Chacha20Poly1305];

    pub fn as_str(self) -> &'static str {
        match self {
            Cipher::AesGcm128 => "AES_128_GCM",
            Cipher::AesGcm256 => "AES_256_GCM",
            Cipher::Chacha20Poly1305 => "CHACHA20_POLY1305",
        }
    }
}

impl CryptoState {
    /// Build the state for one direction. `iv` is the 12-byte IV rustls
    /// extracts (for AES-GCM: 4-byte salt + 8-byte IV, the kernel's split;
    /// for ChaCha20-Poly1305 the whole 12 bytes). `None` if a length is wrong.
    pub fn new(version: u16, cipher: Cipher, key: &[u8], iv: &[u8], seq: u64) -> Option<CryptoState> {
        let rec_seq = seq.to_be_bytes();
        if iv.len() != 12 {
            return None;
        }
        Some(match cipher {
            Cipher::AesGcm128 => CryptoState::AesGcm128(AesGcm128Box(Box::new(AesGcm128 {
                info: CryptoInfo { version, cipher_type: TLS_CIPHER_AES_GCM_128 },
                iv: iv[4..].try_into().ok()?,
                key: key.try_into().ok()?,
                salt: iv[..4].try_into().ok()?,
                rec_seq,
            }))),
            Cipher::AesGcm256 => CryptoState::AesGcm256(AesGcm256Box(Box::new(AesGcm256 {
                info: CryptoInfo { version, cipher_type: TLS_CIPHER_AES_GCM_256 },
                iv: iv[4..].try_into().ok()?,
                key: key.try_into().ok()?,
                salt: iv[..4].try_into().ok()?,
                rec_seq,
            }))),
            Cipher::Chacha20Poly1305 => CryptoState::Chacha20Poly1305(ChachaBox(Box::new(Chacha20Poly1305 {
                info: CryptoInfo { version, cipher_type: TLS_CIPHER_CHACHA20_POLY1305 },
                iv: iv.try_into().ok()?,
                key: key.try_into().ok()?,
                rec_seq,
            }))),
        })
    }

    fn as_bytes(&self) -> (*const libc::c_void, libc::socklen_t) {
        fn raw<T>(v: &T) -> (*const libc::c_void, libc::socklen_t) {
            (v as *const T as *const libc::c_void, std::mem::size_of::<T>() as libc::socklen_t)
        }
        match self {
            CryptoState::AesGcm128(b) => raw(&*b.0),
            CryptoState::AesGcm256(b) => raw(&*b.0),
            CryptoState::Chacha20Poly1305(b) => raw(&*b.0),
        }
    }
}

fn setsockopt(fd: RawFd, level: libc::c_int, name: libc::c_int, value: *const libc::c_void, len: libc::socklen_t) -> io::Result<()> {
    // SAFETY: `value` points to `len` readable bytes for the duration of the call.
    let rc = unsafe { libc::setsockopt(fd, level, name, value, len) };
    if rc == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

/// Attach the `tls` upper-layer protocol. Until a direction gets its keys
/// the socket passes bytes through unchanged.
pub fn set_ulp(fd: RawFd) -> io::Result<()> {
    const NAME: &[u8] = b"tls";
    setsockopt(fd, libc::SOL_TCP, libc::TCP_ULP, NAME.as_ptr().cast(), NAME.len() as libc::socklen_t)
}

/// Install one direction's keys (`TLS_TX` or `TLS_RX`).
pub fn set_crypto(fd: RawFd, direction: libc::c_int, state: &CryptoState) -> io::Result<()> {
    let (ptr, len) = state.as_bytes();
    setsockopt(fd, SOL_TLS, direction, ptr, len)
}

/// Send one record of `record_type` (an alert, say) on a kTLS socket.
pub fn send_record(fd: RawFd, record_type: u8, data: &[u8]) -> io::Result<usize> {
    // SAFETY: CMSG_SPACE is a pure computation.
    let space = unsafe { libc::CMSG_SPACE(1) } as usize;
    let mut cbuf = [0u64; 4]; // 32 bytes, aligned; CMSG_SPACE(1) is 24 on 64-bit
    debug_assert!(space <= std::mem::size_of_val(&cbuf));
    let mut iov = libc::iovec { iov_base: data.as_ptr() as *mut libc::c_void, iov_len: data.len() };
    // SAFETY: msghdr is plain data; zeroed is a valid empty header.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cbuf.as_mut_ptr().cast();
    msg.msg_controllen = space as _;
    // SAFETY: msg_control points to `space` bytes we own; CMSG_FIRSTHDR
    // returns a pointer inside it.
    unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&msg);
        (*cmsg).cmsg_level = SOL_TLS;
        (*cmsg).cmsg_type = TLS_SET_RECORD_TYPE;
        (*cmsg).cmsg_len = libc::CMSG_LEN(1) as _;
        *libc::CMSG_DATA(cmsg) = record_type;
    }
    // SAFETY: msg is fully initialised above.
    let n = unsafe { libc::sendmsg(fd, &msg, libc::MSG_NOSIGNAL) };
    if n < 0 { Err(io::Error::last_os_error()) } else { Ok(n as usize) }
}

/// `recvmsg` on a kTLS socket: the bytes of one or more records of the same
/// type, and that type. Non-blocking sockets return `WouldBlock`.
pub fn recv_record(fd: RawFd, buf: &mut [u8]) -> io::Result<(usize, u8)> {
    let mut cbuf = [0u64; 4];
    let mut iov = libc::iovec { iov_base: buf.as_mut_ptr().cast(), iov_len: buf.len() };
    // SAFETY: as in send_record.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cbuf.as_mut_ptr().cast();
    msg.msg_controllen = std::mem::size_of_val(&cbuf) as _;
    // SAFETY: buffers live for the call.
    let n = unsafe { libc::recvmsg(fd, &mut msg, 0) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut record_type = RECORD_APPLICATION_DATA;
    // SAFETY: walking the control messages the kernel just wrote.
    unsafe {
        let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
        while !cmsg.is_null() {
            if (*cmsg).cmsg_level == SOL_TLS && (*cmsg).cmsg_type == TLS_GET_RECORD_TYPE {
                record_type = *libc::CMSG_DATA(cmsg);
            }
            cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
        }
    }
    Ok((n as usize, record_type))
}
