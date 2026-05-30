use std::io;
mod object;
mod repo;

pub mod cli;

pub type GitResult<T> = Result<T, GitError>;

#[derive(Debug)]
pub enum GitError {
    IOError(io::Error),
    ObjectError(String),
    CLIError(String),
}

