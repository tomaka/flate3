use std::io::{self, Read};

mod adler32;
mod bit;
mod crc32;
mod huffman;
mod zlib_decoder;

/// Reads in the whole buffer. If an EOF error happens, returns `InvalidInput`.
fn read_all<R>(reader: &mut R, mut output: &mut [u8]) -> io::Result<()> where R: Read {
    debug_assert!(output.len() != 0);

    let mut offset = 0;

    loop {
        match reader.read(&mut output[offset..]) {
            Ok(len) if len == output.len() - offset => {
                return Ok(());
            },
            Ok(0) => return Err(io::Error::new(io::ErrorKind::InvalidInput, "Unexpected EOF")),
            Ok(len) => {
                offset += len;
            },
            Err(e) => return Err(e),
        }
    }
}

#[test]
fn it_works() {
}
