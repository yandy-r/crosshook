#[cfg(test)]
mod discovery;
#[cfg(test)]
mod flatpak;
#[cfg(test)]
mod resolution;

use std::fs;
use std::path::Path;

pub(super) fn create_tool(directory_path: &Path, compatibilitytool_vdf: Option<&str>) {
    fs::create_dir_all(directory_path).expect("tool dir");
    fs::write(directory_path.join("proton"), b"#!/bin/sh\n").expect("proton file");

    if let Some(content) = compatibilitytool_vdf {
        fs::write(directory_path.join("compatibilitytool.vdf"), content).expect("vdf");
    }
}

pub(super) fn write_steam_config(root: &Path, content: &str) {
    let config_dir = root.join("config");
    fs::create_dir_all(&config_dir).expect("config dir");
    fs::write(config_dir.join("config.vdf"), content).expect("config.vdf");
}

pub(super) fn write_userdata_config(root: &Path, user_id: &str, content: &str) {
    let config_dir = root.join("userdata").join(user_id).join("config");
    fs::create_dir_all(&config_dir).expect("userdata config dir");
    fs::write(config_dir.join("localconfig.vdf"), content).expect("localconfig.vdf");
}
