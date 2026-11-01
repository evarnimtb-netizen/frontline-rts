//! `desync-diff` through the real binary: identical saves exit 0, saves
//! of two diverged worlds exit 1 and name the fields.

use std::path::{Path, PathBuf};
use std::process::Command;

use frontline_cli::SkirmishArgs;
use frontline_cli::skirmish::match_setup;
use frontline_sim::{Command as SimCommand, CommandKind, PlayerId, World};

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("frontline-cli-desync");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn diff(a: &Path, b: &Path) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_frontline-cli"))
        .args(["desync-diff"])
        .arg(a)
        .arg(b)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn desync_diff_names_diverged_fields() {
    let setup = match_setup(&SkirmishArgs::default()).unwrap();
    let mut a = World::new_match(setup.clone(), 1).unwrap();
    let mut b = World::new_match(setup, 1).unwrap();
    for _ in 0..60 {
        a.step(&[]);
        b.step(&[]);
    }
    let (pa, pb) = (tmp("a.sav"), tmp("b.sav"));
    std::fs::write(&pa, a.save()).unwrap();
    std::fs::write(&pb, b.save()).unwrap();
    let (code, text) = diff(&pa, &pb);
    assert_eq!(code, 0, "{text}");
    assert!(text.starts_with("saves are identical"), "{text}");

    // One extra order on one side only: player 0 surrenders.
    b.step(&[SimCommand {
        player: PlayerId(0),
        seq: 0,
        kind: CommandKind::Surrender,
    }]);
    a.step(&[]);
    std::fs::write(&pa, a.save()).unwrap();
    std::fs::write(&pb, b.save()).unwrap();
    let (code, text) = diff(&pa, &pb);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("player 0 surrendered: false vs true"),
        "{text}"
    );
    assert!(text.contains("stats.commands_applied"), "{text}");
}
