use std::io::Read;

use super::*;

#[test]
fn the_trailer_frame_is_flagged_sized_and_lower_case() {
    let frame = trailer_frame(&[Header::new("Grpc-Status", "0".to_string()), Header::new("grpc-message", " ok ".to_string())]);
    let lines = b"grpc-status:0\r\ngrpc-message:ok\r\n";
    assert_eq!(0x80, frame[0]);
    assert_eq!(lines.len() as u32, u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]));
    assert_eq!(lines.as_slice(), &frame[5..]);
    assert!(trailer_frame(&[]).is_empty());
}

/// A reader handing out `chunks` one per read, as a network stream would.
struct Chunks(Vec<Vec<u8>>);

impl Read for Chunks {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.0.is_empty() {
            return Ok(0);
        }
        let chunk = self.0.remove(0);
        out[..chunk.len()].copy_from_slice(&chunk);
        Ok(chunk.len())
    }
}

fn read_all(mut reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    reader.read_to_end(&mut out)?;
    Ok(out)
}

#[test]
fn base64_streams_encode_like_the_whole_body_whatever_the_chunks() {
    let body: Vec<u8> = (0..200u8).collect();
    let whole = Base64::encode(&body).unwrap().into_bytes();
    for sizes in [vec![1, 2, 3, 194], vec![100, 100], vec![7, 7, 7, 179], vec![200]] {
        let mut chunks = Vec::new();
        let mut at = 0;
        for size in sizes {
            chunks.push(body[at..at + size].to_vec());
            at += size;
        }
        let encoded = read_all(Base64Encoder::new(Box::new(Chunks(chunks.clone())))).unwrap();
        assert_eq!(whole, encoded);
        let decoded = read_all(Base64Decoder::new(Box::new(Chunks(encoded.chunks(5).map(|c| c.to_vec()).collect())))).unwrap();
        assert_eq!(body, decoded);
    }
}

#[test]
fn text_bodies_may_be_several_padded_segments() {
    let segments = [Base64::encode(b"ab").unwrap(), Base64::encode(b"cdef").unwrap()].concat();
    assert_eq!(b"abcdef".to_vec(), decode_text(segments.as_bytes()).unwrap());
    let decoded = read_all(Base64Decoder::new(Box::new(Chunks(vec![segments.into_bytes()])))).unwrap();
    assert_eq!(b"abcdef".to_vec(), decoded);
    assert!(decode_text(b"abc").is_err());
    assert!(read_all(Base64Decoder::new(Box::new(Chunks(vec![b"abc".to_vec()])))).is_err());
}

#[test]
fn the_trailer_frame_follows_a_streamed_body() {
    let trailers = Trailers::new();
    let reader = TrailerFrame { body: Box::new(Chunks(vec![b"msg".to_vec()])), trailers: Some(trailers.clone()), frame: None };
    trailers.set(vec![Header::new("grpc-status", "0".to_string())]);
    let out = read_all(reader).unwrap();
    assert_eq!([b"msg".as_slice(), &trailer_frame(&trailers.get())].concat(), out);
}
