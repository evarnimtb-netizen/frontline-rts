//! Stamps the build id parts the wire `Hello` carries (S2 "Session setup"):
//! the git commit with a `-dirty` flag, and the sim's save version.
//!
//! `FRONTLINE_BUILD_COMMIT` overrides the commit (the export script sets it
//! when it builds from a clean checkout elsewhere). The save version is read
//! from the sim source, because the sim keeps `SAVE_VERSION` private and S2
//! may not change the sim; the build fails if the line ever moves.

use std::path::Path;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn main() {
    println!("cargo:rerun-if-env-changed=FRONTLINE_BUILD_COMMIT");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let root = Path::new(&manifest).join("../..");
    // Re-run when HEAD moves or the index changes (a commit, a checkout or
    // staging). Edits to tracked files without staging are not seen until
    // something else rebuilds this crate; the dirty flag is a hint.
    for f in [".git/HEAD", ".git/index"] {
        let p = root.join(f);
        if p.exists() {
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }
    let commit = match std::env::var("FRONTLINE_BUILD_COMMIT") {
        Ok(c) if !c.is_empty() => c,
        _ => {
            let root_s = root.to_string_lossy().into_owned();
            match git(&["-C", &root_s, "rev-parse", "--short=12", "HEAD"]) {
                Some(c) => {
                    let clean = Command::new("git")
                        .args(["-C", &root_s, "diff", "--quiet", "HEAD", "--"])
                        .status()
                        .is_ok_and(|s| s.success());
                    if clean { c } else { format!("{c}-dirty") }
                }
                None => "unknown".to_owned(),
            }
        }
    };
    println!("cargo:rustc-env=FRONTLINE_GIT_COMMIT={commit}");

    let world = root.join("crates/sim/src/world.rs");
    println!("cargo:rerun-if-changed={}", world.display());
    let src = std::fs::read_to_string(&world).expect("reading crates/sim/src/world.rs");
    let version = src
        .lines()
        .find_map(|l| l.trim().strip_prefix("const SAVE_VERSION: u32 = "))
        .and_then(|v| v.strip_suffix(';'))
        .and_then(|v| v.trim().parse::<u32>().ok())
        .expect("crates/sim/src/world.rs no longer has `const SAVE_VERSION: u32 = N;`");
    println!("cargo:rustc-env=FRONTLINE_SAVE_VERSION={version}");
}
