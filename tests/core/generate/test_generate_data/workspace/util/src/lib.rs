include!(concat!(env!("OUT_DIR"), "/major.rs"));

pub fn describe() -> String {
    format!(
        "{} {} major={} build_script={} description={} origin={}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        MAJOR,
        env!("FROM_BUILD_SCRIPT"),
        env!("CARGO_PKG_DESCRIPTION"),
        // A file in a hidden directory, which the crate reads as Cargo lets it.
        include_str!("../.config/origin.txt"),
    )
}
