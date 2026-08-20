use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no tsconfig.json at {0}")]
    NoTsConfig(PathBuf),
    #[error("{0}")]
    Resolve(String),
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "tsgo not found; install TypeScript 7 or set SLOPGRAPH_TSGO to the native tsc/tsgo binary"
    )]
    TsgoNotFound,
    #[error("tsgo: {0}")]
    Tsgo(String),
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
