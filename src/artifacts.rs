use std::path::{Path, PathBuf};

use crate::Error;

pub fn validate_artifact_path(_root: &Path, _rel: &str) -> Result<PathBuf, Error> {
    todo!()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::validate_artifact_path;

    #[test]
    fn rejects_parent_traversal() {
        let root = tempdir().unwrap();
        assert!(validate_artifact_path(root.path(), "../x").is_err());
    }

    #[test]
    fn rejects_absolute_paths() {
        let root = tempdir().unwrap();
        assert!(validate_artifact_path(root.path(), "/etc/passwd").is_err());
    }

    #[test]
    fn rejects_nested_parent_traversal() {
        let root = tempdir().unwrap();
        assert!(validate_artifact_path(root.path(), "a/../../b").is_err());
    }

    #[test]
    fn rejects_empty_paths() {
        let root = tempdir().unwrap();
        assert!(validate_artifact_path(root.path(), "").is_err());
    }

    #[test]
    fn accepts_bundle_relative_artifact_paths() {
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("artifacts")).unwrap();
        fs::write(root.path().join("artifacts/results.json"), "{}").unwrap();
        assert_eq!(
            validate_artifact_path(root.path(), "artifacts/results.json").unwrap(),
            root.path().join("artifacts/results.json")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_escape_the_bundle() {
        use std::os::unix::fs::symlink;

        let root = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::create_dir(root.path().join("artifacts")).unwrap();
        fs::write(outside.path().join("secret.txt"), "outside").unwrap();
        symlink(
            outside.path().join("secret.txt"),
            root.path().join("artifacts/escape.txt"),
        )
        .unwrap();
        assert!(validate_artifact_path(root.path(), "artifacts/escape.txt").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn rejects_symlinks_that_escape_the_bundle() {
        use std::process::Command;

        let root = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::create_dir(root.path().join("artifacts")).unwrap();
        fs::write(outside.path().join("secret.txt"), "outside").unwrap();
        let link = root.path().join("artifacts/escape");
        let link = link.display().to_string().replace('\'', "''");
        let target = outside.path().display().to_string().replace('\'', "''");
        let script =
            format!("New-Item -ItemType Junction -Path '{link}' -Target '{target}' | Out-Null");
        let junction = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(script)
            .status()
            .unwrap();
        assert!(junction.success(), "failed to create test junction");
        assert!(validate_artifact_path(root.path(), "artifacts/escape/secret.txt").is_err());
    }
}
