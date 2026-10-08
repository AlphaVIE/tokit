//! `tok add <name> --git <url>` pins a repository commit and tree digest,
//! stores the package, rebuilds the store entry offline-first, and rejects
//! tampered pins.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn git(directory: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Tokit Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "core.autocrlf=false", "-c", "init.defaultBranch=main"])
        .arg("-C")
        .arg(directory)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn tok(directory: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(args)
        .current_dir(directory)
        .env("TOK_HOME", home)
        .output()
        .unwrap()
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

fn repository(base: &Path) -> PathBuf {
    let repo = base.join("greet-repo");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(
        repo.join("tok.toml"),
        "[package]\nname = \"greet\"\nversion = \"0.1.0\"\nentry = \"src/greet.tok\"\ndescription = \"greetings\"\n\n[dependencies]\n",
    )
    .unwrap();
    std::fs::write(
        repo.join("src/greet.tok"),
        "pub hello(name:String)->String{\"hello \"+name}",
    )
    .unwrap();
    std::fs::write(repo.join("README.md"), "not part of the package").unwrap();
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "first"]);
    repo
}

#[test]
fn git_dependencies_are_pinned_stored_and_verified() {
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let base = std::env::temp_dir().join(format!("tokit-git-{}", common::nonce()));
    std::fs::create_dir_all(&base).unwrap();
    let home = base.join("home");
    let repo = repository(&base);
    let url = repo.to_str().unwrap().replace('\\', "/");
    assert!(tok(&base, &home, &["new", "app"]).status.success());
    let app = base.join("app");
    let added = tok(&app, &home, &["add", "greet", "--git", &url]);
    assert!(added.status.success(), "{}", text(&added));
    let manifest = std::fs::read_to_string(app.join("tok.toml")).unwrap();
    assert!(manifest.contains(&format!("git = \"{url}\"")), "{manifest}");
    let rev = manifest
        .split("rev = \"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    assert_eq!(rev.len(), 40);
    std::fs::write(
        app.join("main.tok"),
        "import g=\"pkg:greet\";\nmain()->String{g::hello(\"tokit\")}",
    )
    .unwrap();
    let run = tok(&app, &home, &["run", "main.tok"]);
    assert_eq!(text(&run), "\"hello tokit\"");

    // A new upstream commit does not change the pinned build.
    std::fs::write(
        repo.join("src/greet.tok"),
        "pub hello(name:String)->String{\"changed \"+name}",
    )
    .unwrap();
    git(&repo, &["commit", "--quiet", "-am", "second"]);
    // Losing the store entry refetches the pinned commit.
    std::fs::remove_dir_all(home.join("store")).unwrap();
    assert_eq!(
        text(&tok(&app, &home, &["run", "main.tok"])),
        "\"hello tokit\""
    );
    // Only tok.toml and .tok files enter the store.
    let stored: Vec<_> = std::fs::read_dir(home.join("store"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(stored.len(), 1);
    assert!(!stored[0].join("README.md").exists());

    // A tampered digest fails: the refetched tree does not match it.
    let digest = manifest
        .split("sha256 = \"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    std::fs::write(
        app.join("tok.toml"),
        manifest.replace(&digest, &"a".repeat(64)),
    )
    .unwrap();
    std::fs::remove_dir_all(home.join("store")).unwrap();
    assert!(text(&tok(&app, &home, &["run", "main.tok"])).contains("pins"));
    // A commit that does not exist cannot be fetched.
    std::fs::write(
        app.join("tok.toml"),
        manifest.replace(&rev, &"0".repeat(40)),
    )
    .unwrap();
    std::fs::remove_dir_all(home.join("store")).unwrap();
    let broken = tok(&app, &home, &["run", "main.tok"]);
    assert!(!broken.status.success());
    // A branch name is not a pin.
    std::fs::write(app.join("tok.toml"), manifest.replace(&rev, "main")).unwrap();
    assert!(text(&tok(&app, &home, &["run", "main.tok"])).contains("full commit id"));
    // Adding at an explicit revision picks that commit's content.
    std::fs::write(app.join("tok.toml"), "[dependencies]\n").unwrap();
    let pinned = tok(&app, &home, &["add", "greet", "--git", &url, "--rev", &rev]);
    assert!(pinned.status.success(), "{}", text(&pinned));
    assert_eq!(
        text(&tok(&app, &home, &["run", "main.tok"])),
        "\"hello tokit\""
    );
    std::fs::remove_dir_all(&base).ok();
}
