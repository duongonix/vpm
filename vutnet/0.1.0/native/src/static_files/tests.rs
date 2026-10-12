use std::fs;

use cap_std::{ambient_authority, fs::Dir};
use tempfile::TempDir;

use super::{StaticFiles, path};

#[test]
fn rejects_url_and_windows_path_escape_forms() {
    for value in [
        "../secret",
        "%2e%2e/secret",
        "foo/../../secret",
        "foo/%2e%2e/secret",
        "%2fsecret",
        "%5csecret",
        "C:%5csecret",
        "foo\\..\\secret",
        "/secret",
        "foo//secret",
        ".",
        "foo/./secret",
        "foo/%00",
        "%xx",
        "CON",
        "nul.txt",
        "COM1",
        "LPT²",
        "file:stream",
        ".. ",
        "trailing.",
        "%ff",
    ] {
        assert!(path::relative(value).is_err(), "accepted {value}");
    }
    assert_eq!(
        path::relative("css/a%20b.css").unwrap(),
        std::path::PathBuf::from("css/a b.css")
    );
}

#[test]
fn validates_mount_prefixes_and_roots_before_listening() {
    let directory = TempDir::new().unwrap();
    let root = directory.path().to_string_lossy().into_owned();
    assert!(StaticFiles::open(vec![("/assets".into(), root.clone())]).is_ok());
    assert!(
        StaticFiles::open(vec![
            ("/assets".into(), root.clone()),
            ("/assets/".into(), root.clone())
        ])
        .is_err()
    );
    for prefix in [
        "",
        "assets",
        "/../secret",
        "/a//b",
        "/a?q=1",
        "/a\\b",
        "/:id",
    ] {
        assert!(StaticFiles::open(vec![(prefix.into(), root.clone())]).is_err());
    }
    assert!(StaticFiles::open(vec![("/assets".into(), root + "/missing")]).is_err());
}

#[test]
fn directory_capability_rejects_parent_and_reparse_escape() {
    let directory = TempDir::new().unwrap();
    let root = directory.path().join("public");
    let outside = directory.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "must not be served").unwrap();
    let cap = Dir::open_ambient_dir(&root, ambient_authority()).unwrap();
    assert!(cap.open("../outside/secret.txt").is_err());
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
    #[cfg(windows)]
    {
        // Directory junctions need no symlink privilege on Windows.
        let result = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(root.join("escape"))
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "junction creation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    #[cfg(any(unix, windows))]
    assert!(
        cap.open("escape/secret.txt").is_err(),
        "escaped through a symlink/reparse point"
    );
}
