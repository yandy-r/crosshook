use std::fs;
use std::path::Path;

pub(super) fn write_steam_client_root(path: &Path) {
    fs::create_dir_all(path.join("steamapps")).expect("steamapps");
    fs::create_dir_all(path.join("config")).expect("config");
}
