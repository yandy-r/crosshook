use crate::protonup::ProtonUpAvailableVersion;

pub(super) fn minimal_ge_proton_tar_gz(tool_dir_name: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let encoder = flate2::write::GzEncoder::new(&mut buf, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header
            .set_path(format!("{tool_dir_name}/proton"))
            .expect("tar path");
        header.set_size(0);
        header.set_cksum();
        builder
            .append(&header, &mut std::io::empty())
            .expect("append empty proton");
        builder.finish().expect("tar finish");
    }
    buf
}

pub(super) fn make_version(
    version: &str,
    download_url: Option<&str>,
    checksum_url: Option<&str>,
) -> ProtonUpAvailableVersion {
    ProtonUpAvailableVersion {
        provider: "ge-proton".to_string(),
        version: version.to_string(),
        release_url: None,
        download_url: download_url.map(str::to_string),
        checksum_url: checksum_url.map(str::to_string),
        checksum_kind: Some("sha512".to_string()),
        asset_size: None,
        published_at: None,
    }
}

pub(super) fn sha256_version(
    version: &str,
    download_url: Option<&str>,
    checksum_url: Option<&str>,
) -> ProtonUpAvailableVersion {
    ProtonUpAvailableVersion {
        provider: "fake-sha256".to_string(),
        version: version.to_string(),
        release_url: None,
        download_url: download_url.map(str::to_string),
        checksum_url: checksum_url.map(str::to_string),
        checksum_kind: Some("sha256-manifest".to_string()),
        asset_size: None,
        published_at: None,
    }
}
