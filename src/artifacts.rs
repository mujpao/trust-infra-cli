use crate::Error;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::{fs::File, io};

pub fn validate_artifact_path(root: &Path, rel: &str) -> Result<PathBuf, Error> {
    if rel.is_empty() {
        return Err(Error::UnsafePath(format!("path is empty")));
    }

    let path = PathBuf::from(rel);

    if path.is_absolute() {
        return Err(Error::UnsafePath(format!("path {rel} is absolute")));
    }

    for component in path.components() {
        if !matches!(component, Component::Normal(..)) {
            println!("here found absolute path");
            return Err(Error::UnsafePath(format!("path {rel} is absolute")));
        }
    }

    let mut resolved = root.to_path_buf();
    for component in path.components() {
        resolved.push(component.as_os_str());
        let metadata = std::fs::symlink_metadata(&resolved)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::UnsafePath(format!("path {rel} contains a symlink")));
        }
    }

    Ok(std::fs::canonicalize(resolved)?)
}

pub fn hash_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::validate_artifact_path;
    use crate::Error;

    fn bundle() -> tempfile::TempDir {
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("artifacts")).unwrap();
        fs::write(root.path().join("artifacts/results.json"), b"data").unwrap();
        root
    }

    #[test]
    fn accepts_artifact_path_inside_bundle() {
        let root = bundle();
        let canonical_root = fs::canonicalize(root.path()).unwrap();

        match validate_artifact_path(root.path(), "artifacts/results.json") {
            Ok(path) => {
                assert!(path.starts_with(&canonical_root));
                assert!(path.ends_with("results.json"));
            }
            Err(error) => panic!("expected an accepted artifact path, got {error:?}"),
        }
    }

    // Models direct parent-directory traversal.
    #[test]
    fn rejects_artifact_parent_traversal() {
        let root = bundle();
        assert!(matches!(
            validate_artifact_path(root.path(), "../x"),
            Err(Error::UnsafePath(_))
        ));
    }

    // Models an absolute path that bypasses the bundle root.
    #[test]
    fn rejects_absolute_artifact_path() {
        let root = bundle();
        let res = validate_artifact_path(root.path(), "/etc/passwd");
        assert!(matches!(res, Err(Error::UnsafePath(_))));
    }

    // Models nested parent-directory traversal.
    #[test]
    fn rejects_nested_artifact_parent_traversal() {
        let root = bundle();
        assert!(matches!(
            validate_artifact_path(root.path(), "a/../../b"),
            Err(Error::UnsafePath(_))
        ));
    }

    // Models an empty untrusted path.
    #[test]
    fn rejects_empty_artifact_path() {
        let root = bundle();
        assert!(matches!(
            validate_artifact_path(root.path(), ""),
            Err(Error::UnsafePath(_))
        ));
    }

    // Models a non-Normal current-directory component.
    #[test]
    fn rejects_artifact_current_directory_component() {
        let root = bundle();
        assert!(matches!(
            validate_artifact_path(root.path(), "./artifacts/results.json"),
            Err(Error::UnsafePath(_))
        ));
    }

    // Models parent traversal that resolves back inside the bundle.
    #[test]
    fn rejects_artifact_parent_component_that_resolves_inside_bundle() {
        let root = bundle();
        assert!(matches!(
            validate_artifact_path(root.path(), "artifacts/../artifacts/results.json"),
            Err(Error::UnsafePath(_))
        ));
    }

    #[cfg(unix)]
    // Models a file symlink escaping to a file outside the bundle.
    #[test]
    fn rejects_artifact_file_symlink_to_outside_bundle() {
        use std::os::unix::fs::symlink;

        let root = bundle();
        let outside = tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), b"outside").unwrap();
        symlink(
            outside.path().join("secret.txt"),
            root.path().join("artifacts/link.json"),
        )
        .unwrap();
        assert!(matches!(
            validate_artifact_path(root.path(), "artifacts/link.json"),
            Err(Error::UnsafePath(_))
        ));
    }

    #[cfg(unix)]
    // Models a file symlink to an otherwise safe file inside the bundle.
    #[test]
    fn rejects_artifact_file_symlink_inside_bundle() {
        use std::os::unix::fs::symlink;

        let root = bundle();
        symlink(
            root.path().join("artifacts/results.json"),
            root.path().join("artifacts/link.json"),
        )
        .unwrap();
        assert!(matches!(
            validate_artifact_path(root.path(), "artifacts/link.json"),
            Err(Error::UnsafePath(_))
        ));
    }

    #[cfg(unix)]
    // Models traversal through a directory symlink to outside the bundle.
    #[test]
    fn rejects_artifact_directory_symlink_to_outside_bundle() {
        use std::os::unix::fs::symlink;

        let root = bundle();
        let outside = tempdir().unwrap();
        fs::write(outside.path().join("results.json"), b"outside").unwrap();
        symlink(outside.path(), root.path().join("linkdir")).unwrap();
        assert!(matches!(
            validate_artifact_path(root.path(), "linkdir/results.json"),
            Err(Error::UnsafePath(_))
        ));
    }

    // Models a missing but otherwise well-formed artifact path.
    #[test]
    fn missing_artifact_returns_not_found_io_error() {
        let root = bundle();
        let result = validate_artifact_path(root.path(), "artifacts/nope.json");
        assert!(matches!(
            &result,
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound
        ));
        assert!(!matches!(result, Err(Error::UnsafePath(_))));
    }

    #[cfg(unix)]
    // Models a sibling-directory prefix escape that defeats string-prefix checks.
    #[test]
    fn rejects_artifact_sibling_prefix_escape() {
        use std::os::unix::fs::symlink;

        let parent = tempdir().unwrap();
        let root = parent.path().join("bundle");
        let sibling = parent.path().join("bundle2");
        fs::create_dir_all(root.join("artifacts")).unwrap();
        fs::create_dir(&sibling).unwrap();
        fs::write(sibling.join("payload.json"), b"sibling").unwrap();
        symlink("../../bundle2", root.join("artifacts/escape")).unwrap();
        assert!(matches!(
            validate_artifact_path(&root, "artifacts/escape/payload.json"),
            Err(Error::UnsafePath(_))
        ));
    }
}

#[cfg(test)]
mod hash_tests {
    use super::*;
    use std::io::Write;

    fn tmp_with(bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(bytes).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn known_vector_abc() {
        let f = tmp_with(b"abc");
        assert_eq!(
            hash_file(f.path()).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn empty_file() {
        let f = tmp_with(b"");
        assert_eq!(
            hash_file(f.path()).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn larger_than_buffer() {
        // Exercises the streaming path (io::copy uses an 8 KiB buffer).
        let f = tmp_with(&vec![0x61u8; 1_000_000]);
        assert_eq!(
            hash_file(f.path()).unwrap(),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn missing_file_is_io_not_found() {
        let err = hash_file(std::path::Path::new("/definitely/not/here")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn output_is_lowercase_hex_64() {
        let f = tmp_with(b"x");
        let h = hash_file(f.path()).unwrap();
        assert_eq!(h.len(), 64);
        assert!(
            h.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        );
    }
}
