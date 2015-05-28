extern crate flate3;

use std::fs;
use std::path::Path;
use std::io::Read;

#[test]
fn fixtures() {
    let path = Path::new("tests/fixture");
    for file in fs::read_dir(&path).unwrap() {
        let file = file.unwrap().path();

        let name = format!("{}", file.display());
        if !name.ends_with("r") {
            println!("Testing {:?}", name);

            let compressed = fs::File::open(&file).unwrap();
            let mut decompressed = fs::File::open(format!("{}r", name)).unwrap();

            let mut decoder = flate3::ZlibDecoder::new(compressed);

            let mut result = Vec::new();
            decoder.read_to_end(&mut result).unwrap();
            let mut expected = Vec::new();
            decompressed.read_to_end(&mut expected).unwrap();

            assert_eq!(result, expected);
        }
    }
}
