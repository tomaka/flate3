use std::cmp;
use std::io::{self, Read};
use std::io::Error as IoError;

/// Reads some data bit per bit.
pub struct BitRead<R> where R: Read {
    /// The `Read` object that the bytes are read from.
    inner: R,

    /// The current cached data being read. This is right-shifted when you call `read`.
    data: u16,

    /// Number of bits remaining to read in `current_byte`. Must be between 0 and 7.
    bits: u8,
}

impl<R> BitRead<R> where R: Read {
    pub fn new(inner: R) -> BitRead<R> {
        BitRead {
            inner: inner,
            data: 0,
            bits: 0,
        }
    }

    /// Reads some bits.
    ///
    /// If the stream reaches EOF, returns an `InvalidInput` error.
    ///
    /// **Warning**: reading two bits can be different from reading one bit then one bit.
    /// For example, if the data is `0b10`, then reading one bit then one bit would give `0` then
    /// `1`, while reading two bits would give `0b10`.
    ///
    pub fn read(&mut self, bits: u8) -> Result<u8, IoError> {
        assert!(bits <= 8);

        if bits > self.bits {
            // making sure that there is enough data in `data`
            let mut data = [0];
            if try!(self.inner.read(&mut data)) == 0 {
                return Err(IoError::new(io::ErrorKind::InvalidInput, "Unexpected EOF in bits \
                                                                      stream"));
            }

            assert!(self.bits <= 8);
            self.data |= (data[0] as u16) << self.bits;
            self.bits += 8;
        }

        Ok(self.read_from_cache(bits))
    }

    /// Reads a number of bits from `data`.
    ///
    /// # Panic
    ///
    /// Panics if `bits` is superior to `self.bits`.
    fn read_from_cache(&mut self, bits: u8) -> u8 {
        assert!(bits <= self.bits);

        let result = self.data & ((1 << bits) - 1);
        self.data >>= bits;
        self.bits -= bits;
        result as u8
    }
}


#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use super::BitRead;

    #[test]
    fn test() {
        let data = Cursor::new(vec![0b01001110, 0b11011000]);
        let mut data = BitRead::new(data);

        assert_eq!(data.read(2).unwrap(), 0b10);
        assert_eq!(data.read(3).unwrap(), 0b011);
        assert_eq!(data.read(1).unwrap(), 0b0);
        assert_eq!(data.read(3).unwrap(), 0b001);
        assert_eq!(data.read(3).unwrap(), 0b100);
        assert_eq!(data.read(4).unwrap(), 0b1101);
    }

    #[test]
    #[should_panic]
    fn too_much() {
        let data = Cursor::new(vec![0b01001110, 0b11011000]);
        let mut data = BitRead::new(data);
        data.read(9).unwrap();
    }
}
