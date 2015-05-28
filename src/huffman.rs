//! Huffman table implementation.
//!
//! Note that this is not a generic implementation, but an implementation that uses the
//! restrictions in place with the deflate encoding.

use bit::BitRead;
use std::io;

/// A huffman table. Contains the code -> symbol decoding system.
///
/// The `S` corresponds to the types of symbols (ie. the result of decoding).
#[derive(Debug, Clone)]
pub struct HuffmanTable<S> {
    // The index of each element corresponds to the pattern that must be matched.
    // For example element `0` corresponds to the bits pattern `000000000`.
    //
    // In addition to this, each element contains the number of bits for this pattern to be
    // matched.
    elements: Vec<Option<(u8, S)>>,

    // Minimum number of bits to read before trying to match any pattern.
    min_bits: u8,
}

impl<S> HuffmanTable<S> where S: Clone {
    /// Generates a table of symbols from lengths.
    ///
    /// You must pass each possible symbol in order, and the corresponding code length.
    ///
    /// # Panic
    ///
    /// Panics if one of the lengths is strictly superior to 9 or equal to 0.
    ///
    pub fn from_lengths<I>(lengths: I) -> HuffmanTable<S> where I: IntoIterator<Item = (S, u8)> {
        let lengths = lengths.into_iter().collect::<Vec<_>>();
        assert!(!lengths.is_empty());

        // array where indices are lengths and values are number of elements of that length
        let bitlen_count = {
            let mut bl = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            for &(_, len) in &lengths {
                bl[len as usize] += 1;
            }
            bl
        };

        // finding the minimum number of bits of pattern
        let min_bits = match bitlen_count.iter().position(|&e| e != 0) {
            Some(pos) => pos as u8,
            None => panic!(),
        };
        assert!(min_bits >= 1);

        // array where indices are lengths and values are the starting values for this length
        let mut next_code = {
            let mut code = 0;
            let mut next_code = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            for bit in (1 .. next_code.len()) {
                code = (code + bitlen_count[bit - 1]) << 1;
                next_code[bit] = code;
            }
            next_code
        };

        // building the real array of elements
        let mut elements = Vec::new();
        for (symbol, len) in lengths {
            assert!(len != 0);

            let code = next_code[len as usize];
            next_code[len as usize] += 1;

            if elements.len() <= code as usize {
                for _ in (0 .. 1 + code as usize - elements.len()) {
                    elements.push(None);
                }
            }

            assert!(elements.len() > code);
            elements[code] = Some((len, symbol));
        }

        HuffmanTable {
            elements: elements,
            min_bits: min_bits,
        }
    }

    /// Reads from a bunch of bits and attempts to decode a next symbol by using the table.
    pub fn decode<R>(&self, input: &mut BitRead<R>) -> io::Result<S> where R: io::Read {
        // we store the list of bits that have been read in a buffer
        let mut buffer = 0;
        for _ in (0 .. self.min_bits) {
            buffer <<= 1;
            buffer |= try!(input.read(1)) as u16;
        }
        let mut num_bits_in_buffer = self.min_bits;

        loop {
            // breaking the loop if we have read too much
            if (1 << num_bits_in_buffer) > self.elements.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Bad huffman data"));
            }

            match &self.elements[buffer as usize] {
                &None => (),
                &Some(ref elem) => {
                    if elem.0 == num_bits_in_buffer {
                        return Ok(elem.1.clone());
                    }
                },
            };

            buffer <<= 1;
            buffer |= try!(input.read(1)) as u16;
            num_bits_in_buffer += 1;
        }
    }
}

#[cfg(test)]
mod test {
    use bit::BitRead;
    use std::io::Cursor;
    use super::HuffmanTable;

    #[test]
    fn decode_rfc1951() {
        // takes the example from RFC1951
        let table = HuffmanTable {
            elements: vec![
                Some((1, 'B')),
                None,
                Some((2, 'A')),
                None,
                None,
                None,
                Some((3, 'C')),
                Some((3, 'D')),
            ],
            min_bits: 1,
        };

        // BAACDC
        let data = vec![0b01101010, 0b00011111];
        let data = Cursor::new(data);
        let mut data = BitRead::new(data);

        assert_eq!(table.decode(&mut data).unwrap(), 'B');
        assert_eq!(table.decode(&mut data).unwrap(), 'A');
        assert_eq!(table.decode(&mut data).unwrap(), 'A');
        assert_eq!(table.decode(&mut data).unwrap(), 'C');
        assert_eq!(table.decode(&mut data).unwrap(), 'D');
        assert_eq!(table.decode(&mut data).unwrap(), 'C');
    }

    #[test]
    fn from_lengths_rfc1951() {
        // "Consider the alphabet ABCDEFGH, with bit lengths (3, 3, 3, 3, 3, 2, 4, 4)."
        let tree = HuffmanTable::from_lengths([
            ('A', 3), ('B', 3), ('C', 3), ('D', 3), ('E', 3), ('F', 2), ('G', 4), ('H', 4)
        ].iter().cloned());

        assert_eq!(tree.elements[0b010], Some((3, 'A')));
        assert_eq!(tree.elements[0b011], Some((3, 'B')));
        assert_eq!(tree.elements[0b100], Some((3, 'C')));
        assert_eq!(tree.elements[0b101], Some((3, 'D')));
        assert_eq!(tree.elements[0b110], Some((3, 'E')));
        assert_eq!(tree.elements[0b00], Some((2, 'F')));
        assert_eq!(tree.elements[0b1110], Some((4, 'G')));
        assert_eq!(tree.elements[0b1111], Some((4, 'H')));
    }
}
