use std::path::{Component, Path, PathBuf};

pub(crate) fn validate_export_component(component: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !component.is_empty(),
        "export path component must not be empty"
    );
    anyhow::ensure!(component != ".", "export path component must not be '.'");
    anyhow::ensure!(component != "..", "export path component must not be '..'");
    anyhow::ensure!(
        !component.contains('\0'),
        "export path component must not contain NUL"
    );
    anyhow::ensure!(
        !component.contains('\\'),
        "export path component must not contain '\\'"
    );
    anyhow::ensure!(
        !component.contains('/'),
        "export path component must not contain '/'"
    );
    Ok(())
}

pub(crate) fn get_export_path(root: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let mut path = root.to_path_buf();
    for part in name.split('/') {
        validate_export_component(part)?;
        path.push(part);
    }
    Ok(path)
}

/// Convert an already canonicalized path to a `/`-joined string.
///
/// Fails on `..`, `.`, non-utf8, `\\` in a component, and (when
/// `must_be_relative`) a root directory component.
pub(crate) fn canonicalized_path_to_string(
    path: impl AsRef<Path>,
    must_be_relative: bool,
) -> anyhow::Result<String> {
    let mut path_str = String::new();
    let parts = path
        .as_ref()
        .components()
        .filter_map(|component| match component {
            Component::Normal(os) => {
                let Some(part) = os.to_str() else {
                    return Some(Err(anyhow::anyhow!("invalid character in path")));
                };
                if !part.contains('/') && !part.contains('\\') {
                    Some(Ok(part))
                } else {
                    Some(Err(anyhow::anyhow!("invalid path component {part:?}")))
                }
            }
            Component::RootDir => {
                if must_be_relative {
                    Some(Err(anyhow::anyhow!("invalid path component {component:?}")))
                } else {
                    path_str.push('/');
                    None
                }
            }
            _ => Some(Err(anyhow::anyhow!("invalid path component {component:?}"))),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let parts = parts.join("/");
    path_str.push_str(&parts);
    Ok(path_str)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{canonicalized_path_to_string, get_export_path, validate_export_component};

    #[test]
    fn canonicalized_relative_joins_with_slash() {
        let joined = PathBuf::from("dir").join("file.bin");
        let s = canonicalized_path_to_string(&joined, true).expect("relative path");
        assert_eq!(s, "dir/file.bin", "components join with /");
    }

    #[test]
    fn canonicalized_rejects_parent_dir() {
        let err = canonicalized_path_to_string(Path::new("foo/../bar"), true)
            .expect_err("parent dir component");
        assert!(
            err.to_string().contains("invalid path component"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn canonicalized_rejects_dot() {
        let err = canonicalized_path_to_string(Path::new("."), true).expect_err("dot component");
        assert!(
            err.to_string().contains("invalid path component"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn canonicalized_rejects_root_when_must_be_relative() {
        let err = canonicalized_path_to_string(Path::new("/abs"), true)
            .expect_err("root dir when relative required");
        assert!(
            err.to_string().contains("invalid path component"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn canonicalized_allows_root_when_not_required() {
        let s = canonicalized_path_to_string(Path::new("/abs/file"), false).expect("absolute path");
        assert_eq!(s, "/abs/file", "absolute path keeps leading slash");
    }

    #[test]
    fn canonicalized_rejects_backslash_in_component() {
        let err = canonicalized_path_to_string(Path::new("foo\\bar"), true)
            .expect_err("backslash in component");
        assert!(
            err.to_string().contains("invalid path component"),
            "unexpected error: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn canonicalized_rejects_non_utf8() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let p = Path::new(OsStr::from_bytes(b"foo/\xff\xfe"));
        let err = canonicalized_path_to_string(p, true).expect_err("non-utf8");
        assert!(
            err.to_string().contains("invalid character in path"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn export_rejects_parent_escape() {
        let err = get_export_path(Path::new("/tmp/export"), "foo/../etc/passwd")
            .expect_err("parent escape");
        assert!(err.to_string().contains(".."), "unexpected error: {err}");
    }

    #[test]
    fn export_rejects_empty_dot_nul_backslash() {
        let root = Path::new("/tmp/export");
        get_export_path(root, "").expect_err("empty");
        get_export_path(root, ".").expect_err("dot");
        get_export_path(root, "foo\0bar").expect_err("nul");
        get_export_path(root, "foo\\bar").expect_err("backslash");
        get_export_path(root, "foo//bar").expect_err("empty component from //");
    }

    #[test]
    fn export_rejects_slash_inside_component() {
        let err = validate_export_component("foo/bar").expect_err("slash in component");
        assert!(err.to_string().contains('/'), "unexpected error: {err}");
    }

    #[test]
    fn export_joins_safe_relative_name() {
        let got =
            get_export_path(Path::new("/tmp/export"), "dir/file.bin").expect("safe relative name");
        assert_eq!(
            got,
            Path::new("/tmp/export").join("dir").join("file.bin"),
            "safe name is pushed under root"
        );
    }
}
