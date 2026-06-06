use std::{fs, io::Write};

use crate::{GitError, GitResult, gwriteln, object::Object, protocol::GitClient};

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

    /// Clone a repo
    pub(crate) async fn clone_repo<O: Write>(mut out: O, url: &str) -> GitResult<()> {
        Self::init()?;
        let upload_pack = GitClient::repo(&url).upload_pack();
        let resp = upload_pack.exec().await?;
        resp.exec().await?;

        gwriteln!(out, "clone done, updating repo")?;
        Self::update_repo()?;
        Ok(())
    }

    /// Update Repo
    /// Probably not the correct workflow, but find out the tree of head ref
    /// and re-create all files
    fn update_repo() -> GitResult<()> {
        let commit = fs::read_to_string(".git/refs/heads/master").map_err(GitError::IOError)?;

        // parse commit contents to find out tree and parent
        let mut content = Vec::new();
        Object::cat_object_from_hash(&commit, &mut content)?;
        let content = String::from_utf8_lossy(&content).into_owned();
        let Some((_, tree_hash)) = content.lines().next().unwrap().split_once(" ") else {
            panic!("invalid commit content");
        };

        // Extract all blob hashes
        let mut content = Vec::new();
        Object::ls_tree_from_hash(tree_hash, &mut content, false)?;
        let content = String::from_utf8_lossy(&content).into_owned();
        for entry in content.lines() {
            let parts: Vec<_> = entry.split_whitespace().collect();
            let (mode, objtype, hash, name) = (parts[0], parts[1], parts[2], parts[3]);
            let mut objcontent = Vec::new();
            Object::cat_object_from_hash(hash, &mut objcontent)?;
            // let objcontent = String::from_utf8_lossy(&objcontent);
            if objtype == "blob" {
                std::fs::write(name, &objcontent).map_err(GitError::IOError)?;
                println!("+ {}", name);
            }
        }

        Ok(())
    }
}
