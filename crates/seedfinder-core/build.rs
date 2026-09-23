use std::{env, fs, path::PathBuf};

fn main() {
    let directory = "src/probability_tables";
    println!("cargo:rerun-if-changed={directory}");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    for entry in fs::read_dir(directory).expect("probability table directory") {
        let path = entry.expect("probability table entry").path();
        if path.extension().is_none_or(|extension| extension != "bin") {
            continue;
        }
        let bytes = fs::read(&path).expect("read probability table");
        // The original length bounds runtime allocation. Keep the calibrators'
        // checked-in files untouched; only the embedded copies are compressed.
        let length = u32::try_from(bytes.len()).expect("probability table fits in u32");
        let mut packed = length.to_le_bytes().to_vec();
        packed.extend(miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 9));
        fs::write(output.join(path.file_name().unwrap()), packed)
            .expect("write compressed probability table");
    }
}
