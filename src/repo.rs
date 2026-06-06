use std::{fs, io::Write, path::PathBuf};

use crate::{GitError, GitResult, gwriteln, object::Object, protocol::GitClient};

pub(crate) struct Repo;

impl Repo {
    /// Initializes a repo in cwd
    pub(crate) fn init(dir: Option<PathBuf>) -> GitResult<Self> {
        let mut rootdir = std::env::current_dir().map_err(GitError::IOError)?;
        if let Some(dir) = dir {
            rootdir.push(dir);
        }
        fs::create_dir_all(rootdir.join(".git")).map_err(GitError::IOError)?;
        fs::create_dir(rootdir.join(".git/objects")).map_err(GitError::IOError)?;
        fs::create_dir(rootdir.join(".git/refs")).map_err(GitError::IOError)?;
        fs::create_dir(rootdir.join(".git/refs/heads")).map_err(GitError::IOError)?;
        fs::create_dir(rootdir.join(".git/hooks")).map_err(GitError::IOError)?;
        fs::create_dir(rootdir.join(".git/info")).map_err(GitError::IOError)?;
        fs::write(rootdir.join(".git/config"), "").map_err(GitError::IOError)?;
        fs::write(rootdir.join(".git/description"), "").map_err(GitError::IOError)?;
        fs::write(rootdir.join(".git/refs/heads/master"), "").map_err(GitError::IOError)?;
        fs::write(rootdir.join(".git/HEAD"), "ref: refs/heads/master\n")
            .map_err(GitError::IOError)?;
        Ok(Repo)
    }

    /// Clone a repo
    pub(crate) async fn clone_repo<O: Write>(
        mut out: O,
        dir: Option<PathBuf>,
        url: &str,
    ) -> GitResult<()> {
        let mut rootdir = std::env::current_dir().map_err(GitError::IOError)?;
        if let Some(ref dir) = dir {
            rootdir.push(dir);
        }

        Self::init(dir)?;
        let upload_pack = GitClient::repo(&url, &rootdir).upload_pack();
        let resp = upload_pack.exec().await?;
        let head_sha = resp.exec().await?;

        std::fs::write(rootdir.join(".git/refs/heads/master"), head_sha)
            .expect("failed ot update master head");

        gwriteln!(out, "clone done, updating repo")?;
        Self::update_repo(rootdir)?;
        Ok(())
    }

    /// Update Repo
    /// Probably not the correct workflow, but find out the tree of head ref
    /// and re-create all files
    fn update_repo(rootdir: PathBuf) -> GitResult<()> {
        let commit = fs::read_to_string(rootdir.join(".git/refs/heads/master"))
            .expect("failed reading refs/heads/master");

        // parse commit contents to find out tree and parent
        let mut content = Vec::new();
        Object::cat_object_from_hash(&commit, &mut content, Some(PathBuf::from(&rootdir)))
            .expect("failed loading master ref commit");
        let content = String::from_utf8_lossy(&content).into_owned();
        let Some((_, tree_hash)) = content.lines().next().unwrap().split_once(" ") else {
            panic!("invalid commit content");
        };

        Self::update_repo_tree_recursive(tree_hash, &rootdir, PathBuf::from(&rootdir))
    }

    fn update_repo_tree_recursive<'a>(
        hash: &'a str,
        rootdir: &'a PathBuf,
        treedir: PathBuf,
    ) -> GitResult<()> {
        // Extract all blob hashes
        let mut content = Vec::new();
        Object::ls_tree_from_hash(hash, &mut content, false, Some(PathBuf::from(&rootdir)))
            .expect(&format!("failed to read tree: {}", &hash));
        let content = String::from_utf8_lossy(&content).into_owned();

        for entry in content.lines() {
            let parts: Vec<_> = entry.split_whitespace().collect();
            let (_mode, objtype, hash, name) = (parts[0], parts[1], parts[2], parts[3]);
            let mut objcontent = Vec::new();
            Object::cat_object_from_hash(hash, &mut objcontent, Some(PathBuf::from(&rootdir)))
                .expect("failed loaing object from hash");
            if objtype == "blob" {
                std::fs::write(treedir.join(name), &objcontent).expect("failed creating blob file");
                println!("+ {}", name);
            } else if objtype == "tree" {
                let subtree = treedir.join(name);
                std::fs::create_dir_all(&subtree);
                Self::update_repo_tree_recursive(hash, &rootdir, subtree)
                    .expect(&format!("failed at subtree: {}", &name));
            }
        }

        Ok(())
    }
}
