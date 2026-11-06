use crate::files::{FileError, FileResource, Ownership};
use askama::Template;
use std::path::Path;

pub fn ensure_templated_file<T, F>(
    files: &F,
    path: &Path,
    template: &T,
    mode: Option<u32>,
    ownership: &Ownership,
) -> Result<bool, FileError>
where
    T: Template,
    F: FileResource + ?Sized,
{
    let content = template.render().map_err(|e| {
        FileError::Io(std::io::Error::other(format!(
            "Template rendering failed: {e}"
        )))
    })?;

    files.ensure_file(path, content.as_bytes(), mode, ownership)
}
