//! Static mounts are native responses outside the Vut middleware pipeline.

mod path;
#[cfg(test)]
mod tests;

use std::{io, path::Path, sync::Arc};

use cap_std::{ambient_authority, fs::Dir};
use http::{
    HeaderValue, Method, StatusCode,
    header::{ALLOW, CONTENT_LENGTH, CONTENT_TYPE},
};
use server_native::{FileResponse, NativeResponse, NativeResponseFuture, ServerResponse};
use vut_native_sdk::{List, Nominal, Str, Value};

use crate::identity::StaticMountType;

struct Mount {
    prefix: String,
    directory: Arc<Dir>,
}

#[derive(Default)]
pub(crate) struct StaticFiles {
    mounts: Vec<Mount>,
}

impl StaticFiles {
    pub(crate) fn from_entries(
        entries: &Value<List<Nominal<StaticMountType>>>,
    ) -> Result<Self, ()> {
        let mut mounts = Vec::new();
        for index in 0..entries.len().map_err(|_| ())? {
            let entry = entries.get(index).map_err(|_| ())?;
            let prefix = entry.get_field::<Str>("_prefix").map_err(|_| ())?;
            let directory = entry.get_field::<Str>("_directory").map_err(|_| ())?;
            mounts.push((
                prefix.as_str().map_err(|_| ())?.to_owned(),
                directory.as_str().map_err(|_| ())?.to_owned(),
            ));
        }
        Self::open(mounts)
    }

    fn open(mounts: Vec<(String, String)>) -> Result<Self, ()> {
        let mut result = Self::default();
        for (prefix, directory) in mounts {
            result.add_directory(&prefix, Path::new(&directory))?;
        }
        result.sort_mounts();
        Ok(result)
    }

    pub(crate) fn add_directory(&mut self, prefix: &str, directory: &Path) -> Result<(), ()> {
        let prefix = path::prefix(prefix)?;
        if self.mounts.iter().any(|mount| mount.prefix == prefix) {
            return Err(());
        }
        // Resolve trusted configuration once, then keep request access inside
        // this directory capability to prevent path and symlink escapes.
        let root = std::fs::canonicalize(directory).map_err(|_| ())?;
        let directory = Dir::open_ambient_dir(root, ambient_authority()).map_err(|_| ())?;
        self.mounts.push(Mount {
            prefix,
            directory: Arc::new(directory),
        });
        self.sort_mounts();
        Ok(())
    }

    fn sort_mounts(&mut self) {
        self.mounts
            .sort_by_key(|mount| std::cmp::Reverse(mount.prefix.len()));
    }

    pub(crate) fn response(&self, method: &Method, raw_path: &str) -> Option<NativeResponseFuture> {
        let (mount, suffix) = self.mounts.iter().find_map(|mount| {
            let suffix = if mount.prefix == "/" {
                raw_path.strip_prefix('/')?
            } else if raw_path == mount.prefix {
                ""
            } else {
                raw_path.strip_prefix(&mount.prefix)?.strip_prefix('/')?
            };
            Some((mount, suffix))
        })?;
        if method != Method::GET && method != Method::HEAD {
            return Some(Box::pin(async {
                NativeResponse::Buffered(
                    ServerResponse::text(StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed")
                        .with_header(ALLOW, HeaderValue::from_static("GET, HEAD")),
                )
            }));
        }
        // A trailing slash always denotes a directory, without index fallback.
        if suffix.is_empty() || suffix.ends_with('/') {
            return Some(Box::pin(async {
                buffered(StatusCode::NOT_FOUND, "Not Found")
            }));
        }
        let Ok(relative) = path::relative(suffix) else {
            return Some(Box::pin(async {
                buffered(StatusCode::BAD_REQUEST, "Bad Request")
            }));
        };
        let directory = Arc::clone(&mount.directory);
        let head = method == Method::HEAD;
        Some(Box::pin(async move {
            // Capability path resolution is synchronous; keep it off Tokio workers.
            match tokio::task::spawn_blocking(move || open_response(&directory, &relative, head))
                .await
            {
                Ok(Ok(response)) => response,
                Ok(Err(error)) => file_error(&error),
                Err(_) => buffered(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"),
            }
        }))
    }
}

fn open_response(directory: &Dir, path: &Path, head: bool) -> io::Result<NativeResponse> {
    let file = match directory.open(path) {
        Ok(file) => file,
        Err(error) => {
            // Windows rejects an ordinary file open on directories. This
            // capability-relative lookup only selects the error response;
            // successful file content always uses the original opened handle.
            if error.kind() == io::ErrorKind::PermissionDenied
                && directory.metadata(path).is_ok_and(|value| value.is_dir())
            {
                return Ok(buffered(StatusCode::NOT_FOUND, "Not Found"));
            }
            return Err(error);
        }
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Ok(buffered(StatusCode::NOT_FOUND, "Not Found"));
    }
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let content_type = HeaderValue::from_str(mime.as_ref())
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    if head {
        return Ok(NativeResponse::Buffered(
            ServerResponse::new(StatusCode::OK)
                .with_header(CONTENT_TYPE, content_type)
                .with_header(CONTENT_LENGTH, HeaderValue::from(metadata.len())),
        ));
    }
    Ok(NativeResponse::File(FileResponse::new(
        file.into_std(),
        metadata.len(),
        content_type,
    )))
}

fn buffered(status: StatusCode, message: &'static str) -> NativeResponse {
    NativeResponse::Buffered(ServerResponse::text(status, message))
}

fn file_error(error: &io::Error) -> NativeResponse {
    match error.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => {
            buffered(StatusCode::NOT_FOUND, "Not Found")
        }
        io::ErrorKind::PermissionDenied => buffered(StatusCode::FORBIDDEN, "Forbidden"),
        io::ErrorKind::InvalidInput => buffered(StatusCode::BAD_REQUEST, "Bad Request"),
        _ => buffered(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"),
    }
}
