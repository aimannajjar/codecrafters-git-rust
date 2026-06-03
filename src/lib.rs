use std::{fmt::Display, io};
mod object;
mod repo;
mod tree;
mod commit;

pub mod cli;

#[macro_export]
macro_rules! gwrite {
    ($buf:expr, $($tt:tt)*) => {
        write!($buf, $($tt)*).map_err(GitError::IOError)
    };
}

#[macro_export]
macro_rules! gwriteln {
    ($buf:expr, $($tt:tt)*) => {
        writeln!($buf, $($tt)*).map_err(GitError::IOError)
    };
}

pub type GitResult<T> = Result<T, GitError>;

#[derive(Debug)]
pub enum GitError {
    IOError(io::Error),
    ObjectError(String),
    CLIError(String),
}


impl Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::IOError(e) => writeln!(f, "{}", e.to_string()),
            Self::ObjectError(e) => writeln!(f, "{e}"),
            Self::CLIError(e) => writeln!(f, "{e}"),
        }
    }
}
