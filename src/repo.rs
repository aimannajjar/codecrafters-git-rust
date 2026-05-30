use std::fs;
use crate::{GitResult, GitError};

pub(crate) struct Repo;

impl Repo {
    /// Initializes a repo in cwd
    pub(crate) fn init() -> GitResult<Self> {
        fs::create_dir(".git").map_err(GitError::IOError)?;
        fs::create_dir(".git/objects").map_err(GitError::IOError)?;
        fs::create_dir(".git/refs").map_err(GitError::IOError)?;
        fs::create_dir(".git/refs/heads").map_err(GitError::IOError)?;
        fs::create_dir(".git/hooks").map_err(GitError::IOError)?;
        fs::create_dir(".git/info").map_err(GitError::IOError)?;
        fs::write(".git/config", "").map_err(GitError::IOError)?;
        fs::write(".git/description", "").map_err(GitError::IOError)?;
        fs::write(".git/refs/heads/master", "").map_err(GitError::IOError)?;
        fs::write(".git/HEAD", "ref: refs/heads/master\n").map_err(GitError::IOError)?;
        Ok(Repo)
    }
}
