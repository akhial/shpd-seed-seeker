//! Lossless storage for the calibrated binary tables. Each table is inflated
//! once, on demand, and shared by all estimator threads. Search and generation
//! do not decompress tables in their hot paths.
use std::sync::LazyLock;

fn unpack(packed: &[u8]) -> Box<[u8]> {
    let length = u32::from_le_bytes(packed[..4].try_into().unwrap()) as usize;
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&packed[4..], length)
        .expect("valid embedded probability table");
    assert_eq!(bytes.len(), length, "complete embedded probability table");
    bytes.into_boxed_slice()
}

macro_rules! table {
    ($name:ident, $file:literal) => {
        pub(crate) static $name: LazyLock<Box<[u8]>> =
            LazyLock::new(|| unpack(include_bytes!(concat!(env!("OUT_DIR"), "/", $file))));
    };
}

table!(FLOORS, "floors.bin");
table!(FEELING_ROOMS, "feeling_rooms.bin");
table!(SOURCE_COUNTS, "source_counts.bin");
table!(WAND_REPEATS, "wand_repeats.bin");
table!(ARTIFACT_WORLDS, "artifact_worlds.bin");
table!(ARTIFACT_DECKS, "artifact_decks.bin");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_tables_preserve_every_calibrated_byte() {
        // Raw tables are only included in test binaries. This checks the actual
        // build output against the source data, including every f32 bit pattern.
        for (actual, expected) in [
            (&FLOORS, &include_bytes!("floors.bin")[..]),
            (&FEELING_ROOMS, &include_bytes!("feeling_rooms.bin")[..]),
            (&SOURCE_COUNTS, &include_bytes!("source_counts.bin")[..]),
            (&WAND_REPEATS, &include_bytes!("wand_repeats.bin")[..]),
            (&ARTIFACT_WORLDS, &include_bytes!("artifact_worlds.bin")[..]),
            (&ARTIFACT_DECKS, &include_bytes!("artifact_decks.bin")[..]),
        ] {
            assert_eq!(&***actual, expected);
        }
    }
}
