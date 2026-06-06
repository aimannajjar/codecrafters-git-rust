use clap::{Parser, Subcommand};

use crate::{GitError, GitResult, commit::Commit, gwriteln, object::Object, repo::Repo, tree};
use std::{
    io::{self, StdoutLock, Write},
    path::PathBuf,
};

#[derive(Parser, Debug)]
#[command(about, version)]
struct Args {
    #[command(subcommand)]
    command: GitCommand,
}

#[derive(Debug, Subcommand)]
pub enum GitCommand {
    Init,

    CatFile {
        /// pretty print object file's contents
        #[clap(short = 'p')]
        pretty_print: bool,

        /// SHA1 of object to cat
        #[clap(value_name = "OBJECT_HASH")]
        hash: String,
    },

    HashObject {
        #[clap(value_name = "PATH")]
        path: PathBuf,

        #[clap(short = 'w', long = "write")]
        write_object: bool,
    },

    LsTree {
        /// SHA1 of tree to ls
        #[clap(value_name = "TREE_HASH")]
        hash: String,

        #[clap(long = "name-only")]
        name_only: bool,
    },

    WriteTree {
        #[clap(value_name = "PATH")]
        path: Option<PathBuf>,
    },

    CommitTree {
        #[clap(value_name = "TREE_HASH")]
        tree_hash: String,

        #[clap(short = 'm', long = "message")]
        message: String,

        #[clap(short = 'p', long = "parent")]
        parent_commit: Option<String>,
    },

    Clone {
        #[clap(value_name = "URL")]
        url: String,

        #[clap(value_name = "target-dir")]
        dir: Option<PathBuf>,
    }
}

#[derive(Debug)]
pub struct Git<O: Write> {
    command: Option<GitCommand>,
    out: O, // used to stream commands output directly for performance
}

impl Default for Git<StdoutLock<'static>> {
    fn default() -> Self {
        Self {
            command: None,
            out: io::stdout().lock(),
        }
    }
}

impl<'a> Git<&'a mut Vec<u8>> {
    /// Will stream output to bytes buffer output
    /// which you can grab a reference to using get_out
    pub fn with_bytes_buffer(command: GitCommand, out: &'a mut Vec<u8>) -> Self {
        Self {
            command: Some(command),
            out,
        }
    }
}

impl Git<StdoutLock<'static>> {
    pub fn from_env() -> GitResult<Self> {
        let mut git = Git::default();
        git.command = Some(Args::parse().command);
        Ok(git)
    }
}

impl<O: io::Write> Git<O> {
    pub async fn run(self) -> GitResult<()> {
        match self.command {
            Some(GitCommand::Init) => Self::init(self.out),
            Some(GitCommand::WriteTree { path }) => Self::write_tree(self.out, path),
            Some(GitCommand::CatFile {
                ref hash,
                pretty_print,
            }) => Self::cat_file(self.out, &hash, pretty_print),
            Some(GitCommand::HashObject { path, write_object }) => {
                Self::hash_object(self.out, path, write_object)
            }
            Some(GitCommand::LsTree {
                ref hash,
                name_only,
            }) => Self::ls_tree(self.out, &hash, name_only),
            Some(GitCommand::CommitTree {
                ref tree_hash,
                ref message,
                parent_commit,
            }) => Self::commit_tree(self.out, tree_hash, message, parent_commit.as_deref()),
            Some(GitCommand::Clone { url, dir }) => Self::clone_repo(self.out, dir, &url).await,
            None => unreachable!(),
        }
    }

    /// init commnad
    fn init(mut out: O) -> GitResult<()> {
        if let Err(e) = Repo::init(None) {
            return Err(e);
        }
        gwriteln!(out, "Initialized git directory")
    }

    /// cat-file commnad
    fn cat_file(mut out: O, hash: &str, _pretty_print: bool) -> GitResult<()> {
        if let Err(e) = Object::cat_object_from_hash(hash, &mut out, None) {
            return Err(e);
        }
        Ok(())
    }

    /// hash-object commnad
    fn hash_object(mut out: O, path: PathBuf, write: bool) -> GitResult<()> {
        if let Err(e) = Object::hash_object_from_file(path, &mut out, write) {
            return Err(e);
        }
        Ok(())
    }

    /// ls-tree commnad
    fn ls_tree(mut out: O, hash: &str, name_only: bool) -> GitResult<()> {
        if let Err(e) = Object::ls_tree_from_hash(hash, &mut out, name_only, None) {
            return Err(e);
        }
        Ok(())
    }

    /// commit-tree command
    fn commit_tree(
        mut out: O,
        tree: &str,
        message: &str,
        parent_commit: Option<&str>,
    ) -> GitResult<()> {
        Commit::create_commit(
            &mut out,
            tree,
            parent_commit,
            &String::from("John Doe <john@example.com> 1234567890 +0000"),
            message,
        )
    }

    /// recurisvely generate tree objects starting from current working directory
    // todo: limit generation to staged area
    fn write_tree(mut out: O, path: Option<PathBuf>) -> GitResult<()> {
        let hash = match path {
            Some(path) => tree::Tree::write_tree(PathBuf::from(path)),
            None => {
                let path = std::env::current_dir().map_err(GitError::IOError)?;
                tree::Tree::write_tree(PathBuf::from(path))
            }
        }?;

        let hash = const_hex::encode(hash);
        writeln!(out, "{}", hash).map_err(GitError::IOError)?;
        Ok(())
    }

    async fn clone_repo(out: O, dir: Option<PathBuf>, url: &str) -> GitResult<()> {
        Repo::clone_repo(out, dir, url).await
    }

}
