use std::path::PathBuf;

pub fn c_runtime_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("geo_runtime.c")
}
