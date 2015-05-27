//! An Implementation of RFC 1951

use std::io::{ErrorKind, Read};
use std::io::Error as IoError;
use std::io::Result as IoResult;

use bit::BitRead;

pub struct Inflater<R> where R: Read {
    inner: BitRead<R>,

    /// Data in the block.
    block_data: Vec<u8>,
    
    /// If true, then we have read a block header whose `bfinal` value is true, meaning that
    /// this is the last block of the stream.
    last_block: bool,
}

enum InflaterState {
    /// We are outside of any block.
    OutsideBlock {

    },
}

impl<R> Inflater<R> where R: Read {
    /// Assumes that a block starts in `inner` and reads it.
    fn consume_block_start(&mut self) -> IoResult<()> {
        assert!(self.last_block == false);

        // the bfinal bit indicates whether we are at the last block
        let bfinal = try!(self.inner.read(1)) != 0;
        if bfinal {
            self.last_block = true;
        }

        match try!(self.inner.read(2)) {
            // dynamic huffman codes
            0b10 => {

            },

            // fixed huffman codes
            0b01 => {
                // instead of having the two sets of lengths (see previous section), we use
                // lengths defined by the RFC

                let lit_len_alphabet_lengths: Vec<u8> = (0u32..288).map(|i| {
                    match i {
                        0 ... 143 => 8,
                        144 ... 255 => 9,
                        256 ... 279 => 7,
                        280 ... 287 => 8,
                        _ => unreachable!()
                    }
                }).collect();

                let dist_alphabet_lengths: Vec<u8> = (0 .. 32).map(|_| 5).collect();

                
            },

            // block of uncompressed data
            0b00 => {

            },

            // reserved
            0b11 => IoError::new(ErrorKind::InvalidInput, "Reserved block type 0b11"),
            _ => unreachable!()
        }
    }
}

impl Read for Inflater<R> where R: Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {

    }
}
