//! `keel build --image`: writes a daemonless OCI Image Layout for the already
//! built static binary (spec ch19, KDR-0107). One layer (the binary), no base
//! image, no registry access — every byte is a pure function of the binary
//! and the fixed metadata below (spec §19.5).
//!
//! JSON documents here are small and fixed-shape, so they're hand-formatted
//! in a fixed key order rather than pulling in a JSON library: rung 2 of the
//! dependency ladder (root AGENTS.md hard rule 5) — a few `format!` calls do
//! the whole job.

use crate::sha256::hex_digest;
use crate::tar::TarBuilder;
use std::fs;
use std::path::Path;

/// Path the binary is written to inside the image, and its `Entrypoint`.
const ENTRYPOINT_PATH: &str = "app/main";

pub fn write_oci_image(binary: &[u8], out: &Path) -> Result<(), String> {
    let mut layer_tar = TarBuilder::new();
    layer_tar.add_file(ENTRYPOINT_PATH, 0o755, binary);
    let layer_bytes = layer_tar.finish();
    let layer_digest = hex_digest(&layer_bytes);

    let config_bytes = format!(
        r#"{{"architecture":"amd64","config":{{"Entrypoint":["/{ENTRYPOINT_PATH}"],"User":"65532:65532","WorkingDir":"/"}},"os":"linux","rootfs":{{"diff_ids":["sha256:{layer_digest}"],"type":"layers"}}}}"#
    )
    .into_bytes();
    let config_digest = hex_digest(&config_bytes);

    let manifest_bytes = format!(
        r#"{{"config":{{"digest":"sha256:{config_digest}","mediaType":"application/vnd.oci.image.config.v1+json","size":{}}},"layers":[{{"digest":"sha256:{layer_digest}","mediaType":"application/vnd.oci.image.layer.v1.tar","size":{}}}],"mediaType":"application/vnd.oci.image.manifest.v1+json","schemaVersion":2}}"#,
        config_bytes.len(),
        layer_bytes.len(),
    )
    .into_bytes();
    let manifest_digest = hex_digest(&manifest_bytes);

    let index_bytes = format!(
        r#"{{"manifests":[{{"digest":"sha256:{manifest_digest}","mediaType":"application/vnd.oci.image.manifest.v1+json","size":{}}}],"mediaType":"application/vnd.oci.image.index.v1+json","schemaVersion":2}}"#,
        manifest_bytes.len(),
    )
    .into_bytes();

    let oci_layout_bytes = br#"{"imageLayoutVersion":"1.0.0"}"#.to_vec();

    let files: Vec<(String, Vec<u8>)> = vec![
        ("oci-layout".to_string(), oci_layout_bytes),
        ("index.json".to_string(), index_bytes),
        (format!("blobs/sha256/{config_digest}"), config_bytes),
        (format!("blobs/sha256/{manifest_digest}"), manifest_bytes),
        (format!("blobs/sha256/{layer_digest}"), layer_bytes),
    ];

    if out.extension().and_then(|e| e.to_str()) == Some("tar") {
        write_archive(&files, out)
    } else {
        write_directory(&files, out)
    }
}

fn write_directory(files: &[(String, Vec<u8>)], out: &Path) -> Result<(), String> {
    let _ = fs::remove_dir_all(out);
    for (rel, bytes) in files {
        let path = out.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("could not create {parent:?}: {e}"))?;
        }
        fs::write(&path, bytes).map_err(|e| format!("could not write {path:?}: {e}"))?;
    }
    Ok(())
}

/// `oci-archive` form: the same layout files wrapped in one outer tar.
/// ponytail: no explicit directory entries — every extractor this artifact
/// targets (`docker load`, `skopeo copy`, `tar`) creates missing parent
/// directories from a file entry's path; add directory entries if one doesn't.
fn write_archive(files: &[(String, Vec<u8>)], out: &Path) -> Result<(), String> {
    let mut archive = TarBuilder::new();
    for (rel, bytes) in files {
        archive.add_file(rel, 0o644, bytes);
    }
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("could not create {parent:?}: {e}"))?;
        }
    }
    fs::write(out, archive.finish()).map_err(|e| format!("could not write {out:?}: {e}"))
}
