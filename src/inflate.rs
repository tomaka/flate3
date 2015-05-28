//! An Implementation of RFC 1951

use std::io::{ErrorKind, Read};
use std::io::Error as IoError;
use std::io::Result as IoResult;

use bit::BitRead;
use compressed_block_reader::CompressedBlockReader;

/// Reads data from an underlying reader and decodes it.
pub struct Inflater<R> where R: Read {
    /// Since the algorithm can require us to copy previous data in the stream, we have to
    /// keep a cache of the already decoded data.
    output_cache: Vec<u8>,

    /// If this ever becomes `None`, that means an IoError occured somewhere.
    state: Option<InflaterState<R>>,
}

/// State of the inflater.
enum InflaterState<R> where R: Read {
    /// We are outside of any block.
    BeforeBlockStart {
        /// The source of bits.
        data: BitRead<R>,
    },

    /// Uncompressed data
    UncompressedData {
        /// The uncompressed data.
        data: R,

        /// Number of bytes remaining to read from this uncompressed block.
        len: usize,

        /// If true, then we have read a block header whose `bfinal` value is true, meaning that
        /// this is the last block of the stream.
        last_block: bool,
    },

    CompressedData {
        /// The data to read from. Returns EOF at the end of the block.
        data: CompressedBlockReader<R>,

        /// If true, then we have read a block header whose `bfinal` value is true, meaning that
        /// this is the last block of the stream.
        last_block: bool,
    },

    /// We have finished reading the last block and there's nothing left.
    Eof {
        /// The reader, if the user wants to get it back.
        data: R,
    },
}

impl<R> Inflater<R> where R: Read {
    /// Initializes a new inflater.
    pub fn new(inner: R) -> Inflater<R> {
        Inflater {
            output_cache: Vec::with_capacity(32768 + 258),
            state: Some(InflaterState::BeforeBlockStart {
                data: BitRead::new(inner)
            })
        }
    }
}

impl<R> Read for Inflater<R> where R: Read {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match self.state.take() {
            Some(InflaterState::BeforeBlockStart { data }) => {
                self.state = Some(try!(consume_block_start(data)));
                self.read(buf)
            },

            Some(InflaterState::UncompressedData { mut data, len, last_block }) => {
                assert!(len != 0);

                let result = try!(if buf.len() > len {
                    data.read(&mut buf[..len])
                } else {
                    data.read(buf)
                });

                for b in &buf[..result] {
                    self.output_cache.push(*b);
                }

                if result == 0 {
                    Err(IoError::new(ErrorKind::InvalidInput,
                                     "Unexpected EOF inside uncompressed block"))

                } else if result == len {
                    if last_block {
                        self.state = Some(InflaterState::Eof { data: data });
                    } else {
                        self.state = Some(InflaterState::BeforeBlockStart {
                                              data: BitRead::new(data)
                                          });
                    }
                    Ok(result)

                } else {
                    self.state = Some(InflaterState::UncompressedData { data: data,
                                                                        len: len - result,
                                                                        last_block: last_block });
                    Ok(result)
                }
            },

            Some(InflaterState::CompressedData { mut data, last_block }) => {
                let result = try!(data.with_previous_data(&self.output_cache).read(buf));

                for b in &buf[..result] {
                    self.output_cache.push(*b);
                }

                if result == 0 {
                    if last_block {
                        self.state = Some(InflaterState::Eof {
                                              data: data.into_inner().byte_align_unwrap()
                                          });
                    } else {
                        self.state = Some(InflaterState::BeforeBlockStart {
                                              data: data.into_inner()
                                          });
                    }

                    self.read(buf)

                } else {
                    self.state = Some(InflaterState::CompressedData { data: data,
                                                                      last_block: last_block });
                    Ok(result)
                }
            },

            Some(InflaterState::Eof { data }) => {
                self.state = Some(InflaterState::Eof { data: data });
                return Ok(0);
            },

            None => return Err(IoError::new(ErrorKind::InvalidInput,
                                            "I/O errors in the inflater are unrecoverable"))
        }
    }
}

/// Assumes that a block starts at the start of `bits` and initializes the inflater.
fn consume_block_start<R>(mut bits: BitRead<R>) -> IoResult<InflaterState<R>> where R: Read {
    // the bfinal bit indicates whether we are at the last block
    let bfinal = try!(bits.read(1)) != 0;

    // the next two bits correspond to the type of block
    match try!(bits.read(2)) {
        // dynamic huffman codes
        0b10 => {
            // the block starts with two huffman table definitions
            Ok(InflaterState::CompressedData {
                data: try!(CompressedBlockReader::from_dynamic_tables(bits)),
                last_block: bfinal,
            })
        },

        // fixed huffman codes
        0b01 => {
            // instead of having the two sets of lengths (see previous section), we use
            // lengths defined by the RFC
            Ok(InflaterState::CompressedData {
                data: CompressedBlockReader::from_fixed_tables(bits),
                last_block: bfinal,
            })
        },

        // block of uncompressed data
        0b00 => {
            // the rest of the bits must be ignored
            let mut inner = bits.byte_align_unwrap();

            // reading the header of the uncompressed data
            let mut header = [0, 0, 0, 0];
            try!(::read_all(&mut inner, &mut header));

            let (len, nlen) = (((header[1] as u16) << 8) | header[0] as u16,
                               ((header[3] as u16) << 8) | header[2] as u16);

            // nlen must len's one complement
            if nlen != !len {
                return Err(IoError::new(ErrorKind::InvalidInput, "Failed to match nlen and len"));
            }

            Ok(InflaterState::UncompressedData {
                data: inner,
                len: len as usize,
                last_block: bfinal,
            })
        },

        // reserved
        0b11 => Err(IoError::new(ErrorKind::InvalidInput, "Reserved block type 0b11")),
        _ => unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use super::Inflater;
    use std::io::Cursor;
    use std::io::Read;

    #[test]
    fn uncompressed_block() {
        let data = vec![0x1, 5, 0, 0xfa, 0xff, b'h', b'e', b'l', b'l', b'o'];
        let data = Cursor::new(data);

        let mut inflater = Inflater::new(data);

        let mut output = Vec::new();
        inflater.read_to_end(&mut output).unwrap();
        assert_eq!(output, b"hello");
    }

    #[test]
    fn uncompressed_block_too_short() {
        let data = vec![0x1, 5, 0, 0xfa, 0xff, b'h', b'e', b'l'];
        let data = Cursor::new(data);

        let mut inflater = Inflater::new(data);

        let mut output = Vec::new();
        assert!(inflater.read_to_end(&mut output).is_err());
    }

    #[test]
    fn uncompressed_block_wrong_len_nlen() {
        let data = vec![0x1, 5, 0, 0xfb, 0xff, b'h', b'e', b'l', b'l', b'o'];
        let data = Cursor::new(data);

        let mut inflater = Inflater::new(data);

        let mut output = Vec::new();
        assert!(inflater.read_to_end(&mut output).is_err());
    }

    #[test]
    fn compressed_fixed_block_distance() {
        let data = vec![0x73, 0x49, 0x4d, 0xcb, 0x49, 0x2c, 0x49, 0x55, 0x00, 0x11, 0x00];
        let data = Cursor::new(data);

        let mut inflater = Inflater::new(data);

        let mut output = Vec::new();
        inflater.read_to_end(&mut output).unwrap();
        assert_eq!(output, b"Deflate late");
    }

    #[test]
    fn uncompressed_then_compressed_fixed_block_distance() {
        let data = vec![0x0, 5, 0, 0xfa, 0xff, b'h', b'e', b'l', b'l', b'o',
                        0x73, 0x49, 0x4d, 0xcb, 0x49, 0x2c, 0x49, 0x55, 0x00, 0x11, 0x00];
        let data = Cursor::new(data);

        let mut inflater = Inflater::new(data);

        let mut output = Vec::new();
        inflater.read_to_end(&mut output).unwrap();
        assert_eq!(output, b"helloDeflate late");
    }

    #[test]
    fn compressed_fixed_block_distance_then_uncompressed() {
        let data = vec![0x72, 0x49, 0x4d, 0xcb, 0x49, 0x2c, 0x49, 0x55, 0x00, 0x11, 0x80,
                        0x0, 5, 0, 0xfa, 0xff, b'h', b'e', b'l', b'l', b'o'];
        let data = Cursor::new(data);

        let mut inflater = Inflater::new(data);

        let mut output = Vec::new();
        inflater.read_to_end(&mut output).unwrap();
        assert_eq!(output, b"Deflate latehello");
    }
}
