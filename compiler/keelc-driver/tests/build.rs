use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn keel_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_keel"))
}

struct TempKeel {
    source_path: PathBuf,
    binary: PathBuf,
    temp_dir: PathBuf,
}

impl TempKeel {
    fn new(source: &str, name: &str) -> Self {
        let temp_dir =
            env::temp_dir().join(format!("keel-build-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let source_path = temp_dir.join(format!("{name}.keel"));
        fs::write(&source_path, source).unwrap();

        let binary = temp_dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));

        Self {
            source_path,
            binary,
            temp_dir,
        }
    }
}

impl Drop for TempKeel {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.binary);
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

#[test]
fn build_produces_runnable_binary() {
    let source = r#"fn main() {
    print("hello from built binary")
}
"#;
    let program = TempKeel::new(source, "keel_build_smoke");

    let output = Command::new(keel_bin())
        .arg("build")
        .arg(&program.source_path)
        .output()
        .expect("keel build should start");

    assert!(
        output.status.success(),
        "keel build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        program.binary.exists(),
        "built binary should exist at {:?}",
        program.binary
    );

    let run = Command::new(&program.binary)
        .output()
        .expect("built binary should run");
    assert!(run.status.success());
    assert_eq!(
        String::from_utf8_lossy(&run.stdout).trim(),
        "hello from built binary"
    );
}
