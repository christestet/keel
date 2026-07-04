//! End-to-end check of the `keel build` up-to-date cutoff: a second identical
//! build must not touch the binary, and an edited source must rebuild it.
//! Requires the Go toolchain, like every `keel build` (CI runners have it).

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn build(source: &Path) {
    let status = Command::new(env!("CARGO_BIN_EXE_keelc"))
        .arg("build")
        .arg(source)
        .status()
        .expect("keelc runs");
    assert!(status.success(), "keel build failed");
}

#[test]
fn second_build_is_a_verified_no_op_and_edits_rebuild() {
    let dir = env::temp_dir().join(format!("keelc-build-cache-it-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("app.keel");
    let binary = dir.join(format!("app{}", env::consts::EXE_SUFFIX));

    fs::write(
        &source,
        "use std.print\n\nfn main() -> Unit {\n  print(\"one\")\n}\n",
    )
    .unwrap();
    build(&source);
    let first = fs::metadata(&binary).unwrap().modified().unwrap();
    assert!(
        dir.join(format!(".app{}.keelstamp", env::consts::EXE_SUFFIX))
            .exists(),
        "clean build writes a stamp"
    );

    build(&source);
    let second = fs::metadata(&binary).unwrap().modified().unwrap();
    assert_eq!(first, second, "unchanged rebuild must not touch the binary");

    fs::write(
        &source,
        "use std.print\n\nfn main() -> Unit {\n  print(\"two\")\n}\n",
    )
    .unwrap();
    build(&source);
    let rebuilt = fs::read(&binary).unwrap();
    assert!(
        String::from_utf8_lossy(&rebuilt).contains("two"),
        "edited source must produce a rebuilt binary"
    );

    let _ = fs::remove_dir_all(&dir);
}
