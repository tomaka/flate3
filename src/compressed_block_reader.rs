use std::io::{self, Read, Cursor};
use bit::BitRead;
use huffman::HuffmanTable;

/// A reader that allows reading from a compressed block.
pub struct CompressedBlockReader<R> where R: Read {
    data: BitRead<R>,
    eof: bool,
    lit_len_table: HuffmanTable<LitLenSymbol>,
    dist_table: HuffmanTable<u8>,
}

#[derive(Debug, Copy, Clone)]
enum LitLenSymbol {
    Byte(u8),
    Eof,
    Pointer(u8),
}

impl<R> CompressedBlockReader<R> where R: Read {
    /// Reads dynamic tables from the input stream and builds a reader for this block.
    pub fn from_dynamic_tables(mut inner: BitRead<R>) -> io::Result<CompressedBlockReader<R>> {
        let (lit_len_table, dist_table) = try!(read_dynamic_tables(&mut inner));

        Ok(CompressedBlockReader {
            data: inner,
            eof: false,
            lit_len_table: lit_len_table,
            dist_table: dist_table,
        })
    }

    /// Builds a reader for this block that uses fixed huffman tables.
    pub fn from_fixed_tables(inner: BitRead<R>) -> CompressedBlockReader<R> {
        let lit_len_table = HuffmanTable::from_lengths((0u32..288).map(|i| {
            let sym = match i {
                n @ 0 ... 255 => LitLenSymbol::Byte(n as u8),
                256 => LitLenSymbol::Eof,
                n => LitLenSymbol::Pointer((n - 257) as u8)
            };

            let len = match i {
                0 ... 143 => 8,
                144 ... 255 => 9,
                256 ... 279 => 7,
                280 ... 287 => 8,
                _ => unreachable!()
            };

            (sym, len)
        }));

        let dist_table = HuffmanTable::from_lengths((0 .. 32).map(|val| (val, 5)));

        CompressedBlockReader {
            data: inner,
            eof: false,
            lit_len_table: lit_len_table,
            dist_table: dist_table,
        }
    }

    /// Stops decoding and returns the underlying bits reader.
    pub fn into_inner(self) -> BitRead<R> {
        self.data
    }

    /// Starts reading from the block. We need to pass the data previously read from the stream
    /// in case of a pointer in the uncompressed data.
    pub fn with_previous_data<'a>(&'a mut self, cache: &'a [u8]) -> ReadContext<'a, R> {
        ReadContext {
            reader: self,
            data_cache: cache,
        }
    }
}

pub struct ReadContext<'a, R: 'a> where R: Read {
    reader: &'a mut CompressedBlockReader<R>,
    data_cache: &'a [u8],
}

impl<'a, R: 'a> Read for ReadContext<'a, R> where R: Read {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.reader.eof {
            return Ok(0);
        }

        // number of bytes already written to `buf`
        let mut written = 0;

        loop {
            if written == buf.len() {
                return Ok(written);
            }

            // reading a symbol from the input data
            // this symbol doesn't necessarly mean a byte, it can also be an EOF marker or a
            // pointer to a previous element of the output buffer
            let symbol = try!(self.reader.lit_len_table.decode(&mut self.reader.data));

            match symbol {
                LitLenSymbol::Byte(val) => {
                    // byte to copy to the output
                    buf[written] = val;
                    written += 1;
                },

                LitLenSymbol::Eof => {
                    // we reached the end of the block
                    self.reader.eof = true;
                    return Ok(written);
                },

                LitLenSymbol::Pointer(ptr) => {
                    // this means that we need to copy some existing data
                    let length = LENGTHS[ptr as usize] +
                                 try!(self.reader.data.read(EXTRA_LENGTHS[ptr as usize])) as u16;
                    let distance = try!(self.reader.dist_table.decode(&mut self.reader.data));
                    let distance = DISTANCES[distance as usize] +
                                   try!(self.reader.data.read(EXTRA_DISTANCES[distance as usize]))
                                   as u16;

                    let (src, dest) = buf.split_at_mut(written);
                    let (nb, remaining_data) = try!(read_behind(length, distance, src,
                                                                self.data_cache, dest));
                    assert!(remaining_data.len() == 0);     // FIXME: 
                    written += nb;

                    // FIXME: not totally implemented, there's a repeating thingy
                }
            }
        }
    }
}

fn read_dynamic_tables<R>(inner: &mut BitRead<R>)
                          -> io::Result<(HuffmanTable<LitLenSymbol>, HuffmanTable<u8>)>
                          where R: Read
{
    // the dynamic tables start with the number of elements that are following
    let hlit = try!(inner.read(5)) as u16 + 257;
    let hdist = try!(inner.read(5)) + 1;
    let hclen = try!(inner.read(4)) + 4;

    // The second and third tables are the lit/len table and the distances table. They contain
    // the lengths that we need to pass to `HuffmanTable::from_lengths`.
    //
    // The format of the tables is a list of commands represented by the `DecodingCommand` struct.
    #[derive(Debug, Copy, Clone)]
    enum DecodingCommand {
        /// A single code length.
        CodeLength(u8),
        /// Repeat the previous code 3 to 6 times, depending on the value of the new two bits.
        RepeatPrevious,
        /// Repeats the code length `0` for 3 to 10 times, depending on the value of the next
        /// three bits.
        RepeatZeroSmall,
        /// Repeats the code length `0` for 11 to 138 times, depending on the value of the next
        /// seven bits.
        RepeatZeroLarge,
    }

    // However these commands are themselves encoded using a huffman table. This huffman table
    // is the first table and we are going to read it now.
    let decoding_table = {
        // This table contains the code length of each decoding command.
        let mut decoding_codes = vec![0; 19];
        for (_, &code) in (0 .. hclen).zip(&[16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3,
                                             13, 2, 14, 1, 15])
        {
            decoding_codes[code] = try!(inner.read(3));
        }

        HuffmanTable::from_lengths(
            [
                (DecodingCommand::CodeLength(0), decoding_codes[0]),
                (DecodingCommand::CodeLength(1), decoding_codes[1]),
                (DecodingCommand::CodeLength(2), decoding_codes[2]),
                (DecodingCommand::CodeLength(3), decoding_codes[3]),
                (DecodingCommand::CodeLength(4), decoding_codes[4]),
                (DecodingCommand::CodeLength(5), decoding_codes[5]),
                (DecodingCommand::CodeLength(6), decoding_codes[6]),
                (DecodingCommand::CodeLength(7), decoding_codes[7]),
                (DecodingCommand::CodeLength(8), decoding_codes[8]),
                (DecodingCommand::CodeLength(9), decoding_codes[9]),
                (DecodingCommand::CodeLength(10), decoding_codes[10]),
                (DecodingCommand::CodeLength(11), decoding_codes[11]),
                (DecodingCommand::CodeLength(12), decoding_codes[12]),
                (DecodingCommand::CodeLength(13), decoding_codes[13]),
                (DecodingCommand::CodeLength(14), decoding_codes[14]),
                (DecodingCommand::CodeLength(15), decoding_codes[15]),
                (DecodingCommand::RepeatPrevious, decoding_codes[16]),
                (DecodingCommand::RepeatZeroSmall, decoding_codes[17]),
                (DecodingCommand::RepeatZeroLarge, decoding_codes[18]),
            ].iter().filter(|&&(_, len)| len != 0).cloned()
        )
    };

    // Now that we have the decoding table, we can decode the two real tables with it.
    // This is a macro that decodes a table.
    macro_rules! decode {
        ($inner:expr, $len:expr, $map:expr) => ({
            let mut code = None;
            let mut result = Vec::new();

            for _ in (0 .. $len) {
                match try!(decoding_table.decode($inner)) {
                    DecodingCommand::CodeLength(c) => {
                        code = Some(c);
                        result.push(c);
                    },
                    DecodingCommand::RepeatPrevious => {
                        let code = match code {
                            Some(c) => c,
                            None => return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                               "Can't repeat the previous code length as there is \
                                                none"))
                        };

                        for _ in (0 .. 3 + try!($inner.read(2))) {
                            result.push(code);
                        }
                    },
                    DecodingCommand::RepeatZeroSmall => {
                        for _ in (0 .. 3 + try!($inner.read(3))) {
                            result.push(0);
                        }
                    },
                    DecodingCommand::RepeatZeroLarge => {
                        for _ in (0 .. 11 + try!($inner.read(7))) {
                            result.push(0);
                        }
                    },
                }
            }

            HuffmanTable::from_lengths(result.into_iter().filter(|&l| l != 0).enumerate()
                                             .map(|(num, len)| ($map(num), len)))
        })
    }

    let lit_len_table = decode!(inner, hlit, |num: usize| {
        match num {
            n @ 0 ... 255 => LitLenSymbol::Byte(n as u8),
            256 => LitLenSymbol::Eof,
            n => LitLenSymbol::Pointer((n - 257) as u8)
        }
    });

    let dist_table = decode!(inner, hdist, |n: usize| n as u8);

    Ok((lit_len_table, dist_table))
}

/// Reads from the previous data into the destination.
///
/// Returns the size that was written in `dest`, plus any remaining data.
fn read_behind(length: u16, distance: u16, immediate_cache: &[u8], previous_cache: &[u8],
               dest: &mut [u8]) -> io::Result<(usize, Vec<u8>)>
{
    let mut written = 0;

    // building an iterator of the input data
    // FIXME: check overflow
    let reader = Cursor::new(previous_cache).chain(Cursor::new(immediate_cache));
    let mut reader = reader.bytes()
                           .skip(previous_cache.len() + immediate_cache.len() - distance as usize)
                           .take(length as usize)
                           .map(|b| b.unwrap());

    for (src, dest) in reader.by_ref().zip(dest.iter_mut()) {
        *dest = src;
        written += 1;
    }

    Ok((written, reader.collect()))
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
