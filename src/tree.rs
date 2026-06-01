use std::{fmt::Display, io::{BufRead, BufReader, Read, Take, Write}};

use crate::{GitError, GitResult, object::Object};
pub(crate) struct Tree;

// TODO: remove allow unused
#[allow(unused)]
struct TreeEntry {
    mode: usize,
    path: String,
    hash: [u8; 20],
    size: usize,
}

pub(crate) const SHOW_TREE_FLAGS_NAMES_ONLY: u8 = 1;
pub(crate) const SHOW_TREE_FLAGS_FULL: u8 = 1 << 1;

impl Display for TreeEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)
    }
}

impl Tree {
    pub(crate) fn show_tree<R: Read, O: Write>(
        object: Object<R>,
        mut out: O,
        flags: u8,
    ) -> GitResult<()> {
        if flags & SHOW_TREE_FLAGS_FULL != 0 {} // TODO: implement full tree

        let mut tree_size = object.size.unwrap(); // shouldn't panic
        let mut reader = object.reader()?;
        while tree_size > 0 {
            let tree_entry = Self::read_tree_entry(&mut reader)?;
            tree_size = tree_size - tree_entry.size;
            writeln!(&mut out, "{}", tree_entry).map_err(GitError::IOError)?;
        }
        if tree_size != 0 {
            Err(GitError::ObjectError("tree file size entries mistmach".to_string()))
        } else {
            Ok(())
        }
    }

    fn read_tree_entry<R: Read>(reader: &mut Take<BufReader<R>>) -> GitResult<TreeEntry> {
        let mut mode: usize = 0;
        let mut mlen = 0;

        // parse mode
        loop {
            let mut c = [0u8; 1];
            reader.read(&mut c).map_err(GitError::IOError)?;
            if c[0] == b' ' {
                break 
            }
            else if c[0] < b'0' || c[0] > b'7' {
                return Err(GitError::ObjectError(format!("tree object malformed, unexpected byte: {}", c[0])))
            } 
            mode = (mode << 3) + (c[0] - b'0') as usize;
            mlen = mlen + 1;
        }

        // parse path
        let mut path = Vec::new();
        let mut plen = reader.read_until(0, &mut path).map_err(GitError::IOError)?;
        path.pop(); // pop nul byte
        plen = plen - 1;
        let path = String::from_utf8(path).map_err(|_| GitError::ObjectError("a path in tree file was not valid utf8".to_string()))?;

        // parse hash
        let mut hash = [0u8; 20];
        reader.read(&mut hash).map_err(GitError::IOError)?;

        let size = mlen + plen + 20 + 2; // extra 1 for white space after mode, and another for nul byte after path
        Ok(TreeEntry { mode, hash, path, size })
    }
}

