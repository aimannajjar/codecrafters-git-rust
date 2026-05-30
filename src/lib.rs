use std::io;
mod object;
mod repo;

pub mod cli;

type GitResult<T> = Result<T, GitError>;

#[derive(Debug)]
enum GitError {
    IOError(io::Error),
    ObjectError(String),
}

