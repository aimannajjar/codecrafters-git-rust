use std::{fmt::Display, io};
mod object;
mod repo;
mod tree;

pub mod cli;


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
