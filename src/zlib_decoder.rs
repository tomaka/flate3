use std::io::{ErrorKind, Read};
use std::io::Error as IoError;
use inflate::Inflater;

/// A reader that decodes zlib data from an underlying reader.
pub struct ZlibDecoder<R> where R: Read {
    state: Option<ZlibDecoderState<R>>,
}

enum ZlibDecoderState<R> where R: Read {
    // we haven't started doing anything yet
    Start {
        // naked reader where we will read the header from
        reader: R,
    },

    // we are currently reading compressed data
    CompressedData {
        // reader wrapper around the inflater
        reader: Inflater<R>,
    },

    Checksum,
}

impl<R> ZlibDecoder<R> where R: Read {
    /// Builds a new zlib decoder by taking ownership of a reader where the data will be read from.
    pub fn new(reader: R) -> ZlibDecoder<R> {
        ZlibDecoder {
            state: Some(ZlibDecoderState::Start {
                reader: reader,
            })
        }
    }
}

impl<R> Read for ZlibDecoder<R> where R: Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        match self.state.take() {
            Some(ZlibDecoderState::Start { mut reader }) => {
                try!(consume_zlib_header(&mut reader));
                self.state = Some(ZlibDecoderState::CompressedData {
                    reader: Inflater::new(reader),
                });
                self.read(buf)
            },

            Some(ZlibDecoderState::CompressedData { mut reader }) => {
                let result = try!(reader.read(buf));

                if result == 0 {
                    self.state = Some(ZlibDecoderState::Checksum);
                    self.read(buf)

                } else {
                    self.state = Some(ZlibDecoderState::CompressedData { reader: reader });
                    Ok(result)
                }
            },

            Some(ZlibDecoderState::Checksum) => {
                // FIXME: check checksum
                Ok(0)
            },

            None => {
                return Err(IoError::new(ErrorKind::InvalidInput,
                                        "I/O errors in the inflater are unrecoverable"));
            }
        }
    }
}

/// Consumes the Zlib header from the reader and checks that nothing is wrong with it.
fn consume_zlib_header<R>(reader: &mut R) -> Result<(), IoError> where R: Read {
    let (cmf, flg) = {
        let mut header = [0, 0];
        try!(::read_all(reader, &mut header));
        (header[0], header[1])
    };

    // checking compression method
    if (cmf & 0b1111) != 8 {
        return Err(IoError::new(ErrorKind::InvalidInput, "Unsupported zlib compression method"));
    }

    // checking cinfo
    if ((cmf >> 4) & 0b1111) != 7 {
        return Err(IoError::new(ErrorKind::InvalidInput, "Unsupported value for CInfo in \
                                                          zlib header"));
    }

    // checking the value of `fcheck`
    if ((cmf as u16) * 256 + (flg as u16)) % 31 != 0 {
        return Err(IoError::new(ErrorKind::InvalidInput, "Wrong value for zlib header checksum"));
    }

    // if the `fdict` flag is set, there is a dictionnary ID afterwards here
    let fdict = (flg & 0b00100000) != 0;
    if fdict {
        let mut dict = [0, 0, 0, 0];
        try!(::read_all(reader, &mut dict));
        // TODO: is there something to do with this dictionnary? not sure
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ZlibDecoder;
    use std::io::Cursor;
    use std::io::Read;

    #[test]
    fn hello_world() {
        let data = vec![0x78, 0x9c, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x28, 0xcf, 0x2f, 0xca,
                        0x49, 0x01, 0x00, 0x1a, 0x0b, 0x04, 0x5d];
        let data = Cursor::new(data);

        let mut inflater = ZlibDecoder::new(data);

        let mut output = Vec::new();
        inflater.read_to_end(&mut output).unwrap();
        assert_eq!(output, b"hello world");
    }
}
