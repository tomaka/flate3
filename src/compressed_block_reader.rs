use std::io::{self, Read};
use bit::BitRead;
use huffman::HuffmanTable;

/// A reader that allows reading from a compressed block.
pub struct CompressedBlockReader<R> where R: Read {
    data: BitRead<R>,
    eof: bool,
    lit_len_table: HuffmanTable<LitLenSymbol>,
    dist_table: HuffmanTable<u8>,
}

#[derive(Copy, Clone)]
enum LitLenSymbol {
    Byte(u8),
    Eof,
    Pointer(u8),
}

impl<R> CompressedBlockReader<R> where R: Read {
    pub fn new(inner: BitRead<R>) -> CompressedBlockReader<R> {
        unimplemented!();
        /*CompressedBlockReader {
            data: inner,
            eof: false,
        }*/
    }

    pub fn into_inner(self) -> BitRead<R> {
        self.data
    }
}

impl<R> Read for CompressedBlockReader<R> where R: Read {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // number of bytes already written to `buf`
        let mut written = 0;

        loop {
            if written == buf.len() {
                return Ok(written);
            }

            // reading a symbol from the input data
            // this symbol doesn't necessarly mean a byte, it can also be an EOF marker or a
            // pointer to a previous element of the output buffer
            let symbol = try!(self.lit_len_table.decode(&mut self.data));

            match symbol {
                LitLenSymbol::Byte(val) => {
                    // byte to copy to the output
                    buf[written] = val;
                    written += 1;
                },

                LitLenSymbol::Eof => {
                    // we reached the end of the block
                    return Ok(written);
                },

                LitLenSymbol::Pointer(ptr) => {
                    // this means that we need to copy some existing data
                    let length = LENGTHS[ptr as usize] +
                                 try!(self.data.read(EXTRA_LENGTHS[ptr as usize])) as u16;
                    let distance = try!(self.dist_table.decode(&mut self.data));
                    let distance = DISTANCES[distance as usize] +
                                   try!(self.data.read(EXTRA_DISTANCES[distance as usize])) as u16;

                    unimplemented!();
                }
            }
        }
    }
}

const LENGTHS: [u16; 29] = [
    3,  4,  5,   6,   7,   8,   9,  10,  11, 13,
    15, 17, 19,  23,  27,  31,  35,  43,  51, 59,
    67, 83, 99, 115, 131, 163, 195, 227, 258
];

const EXTRA_LENGTHS: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1,
    1, 1, 2, 2, 2, 2, 3, 3, 3, 3,
    4, 4, 4, 4, 5, 5, 5, 5, 0
];

const DISTANCES: [u16; 30] = [
    1,    2,      3,    4,    5,    7,    9,    13,    17,    25,
    33,   49,     65,   97,  129,  193,  257,   385,   513,   769,
    1025,  1537,  2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577
];

const EXTRA_DISTANCES: [u8; 30] = [
    0, 0,  0,  0,  1,  1,  2,  2,  3,  3,
    4, 4,  5,  5,  6,  6,  7,  7,  8,  8,
    9, 9, 10, 10, 11, 11, 12, 12, 13, 13
];
