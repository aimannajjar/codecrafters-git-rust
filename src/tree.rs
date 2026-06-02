use std::{
    fmt::Display, fs, io::{BufRead, BufReader, BufWriter, Read, Take, Write}, os::unix::fs::PermissionsExt
};

use crate::{GitError, GitResult, object::Object};
pub(crate) const SHOW_TREE_FLAGS_NAMES_ONLY: u8 = 1;
pub(crate) const SHOW_TREE_FLAGS_FULL: u8 = 1 << 1;

#[allow(non_camel_case_types)]
type mode_t = u32;
const S_IFDIR: mode_t = 0o04_0000;
const S_IFREG: mode_t = 0o10_0000;
const S_IFLNK: mode_t = 0o12_0000;

/// Builder for TreeEntries, mostly to correctly calculate a tree entry size
struct TreeEntryBuilder {
    mode: mode_t,
    path: Option<String>,
    hash: [u8; 20],
}

impl TreeEntryBuilder {
    fn path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }

    fn mode(mut self, mode: mode_t) -> Self {
        self.mode = mode;
        self
    }

    fn hash(mut self, hash: [u8; 20]) -> Self {
        self.hash = hash;
        self
    }

    fn build(self) -> TreeEntry {
        assert!(self.path.is_some());
        let size = 20 + 2 + self.path.as_ref().unwrap().len() + 0;
        TreeEntry { mode: self.mode, path: self.path.unwrap(), hash: self.hash, size }
    }
}

impl Default for TreeEntryBuilder {
    fn default() -> Self {
        Self { mode: 0, path: None, hash: [0u8; 20] }
    }
}

/// This represents a single entry in a tree
#[allow(unused)]
struct TreeEntry {
    mode: mode_t,
    path: String,
    hash: [u8; 20],
    size: usize,
}

impl Display for TreeEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)
    }
}

impl TreeEntry {
    pub(crate) fn builder() -> TreeEntryBuilder {
        TreeEntryBuilder::default()
    }
}

/// This represents a compelte tree
pub(crate) struct Tree;
impl Tree {


    pub(crate) fn show_tree<R: Read, O: Write>(
        object: Object<R>,
        mut out: O,
        flags: u8,
    ) -> GitResult<()> {

        let mut tree_size = object.size.unwrap(); // shouldn't panic
        let mut reader = object.reader()?;
        while tree_size > 0 {
            let tree_entry = Self::read_tree_entry(&mut reader)?;
            tree_size = tree_size - tree_entry.size;
            if flags & SHOW_TREE_FLAGS_FULL != 0 {
                writeln!(&mut out, "{:06o}\t{}", tree_entry.mode, tree_entry).map_err(GitError::IOError)?;
            } else {
                writeln!(&mut out, "{}", tree_entry).map_err(GitError::IOError)?;
            }
        }
        if tree_size != 0 {
            Err(GitError::ObjectError("tree file size entries mistmach".to_string()))
        } else {
            Ok(())
        }
    }

    fn read_tree_entry<R: Read>(reader: &mut Take<BufReader<R>>) -> GitResult<TreeEntry> {
        let mut mode: mode_t = 0;
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
            mode = (mode << 3) + (c[0] - b'0') as mode_t;
            mlen = mlen + 1;
        }

        // parse path
        let mut path = Vec::new();
        let mut plen = reader.read_until(0, &mut path).map_err(GitError::IOError)?;
        path.pop(); // pop nul byte
        plen = plen - 1;
        let path = String::from_utf8(path).map_err(|_| {
            GitError::ObjectError("a path in tree file was not valid utf8".to_string())
        })?;

        // parse hash
        let mut hash = [0u8; 20];
        reader.read(&mut hash).map_err(GitError::IOError)?;

        let size = mlen + plen + 20 + 2; // extra 1 for white space after mode, and another for nul byte after path
        Ok(TreeEntry { mode, hash, path, size })
    }

    pub(crate) fn write_tree() -> GitResult<()> {
        let buf: Vec<u8> = Vec::new();
        let writer = BufWriter::new(buf);
        Self::write_tree_recursive(writer)
    }

    fn write_tree_recursive<W: Write>(writer: BufWriter<W>) -> GitResult<()> {
        for entry in fs::read_dir("./").map_err(GitError::IOError)? {
            if let Ok(entry) = entry {
                // let tree_entry 
                if entry.path().is_dir() {
                    // TODO: impplement tree recurse
                } else {
                    let mut hash_hex = [0u8; 41];
                    let metadata = entry.path().metadata().map_err(GitError::IOError)?;
                    let mode = S_IFREG | metadata.permissions().mode();
                    let o = Object::hash_object_from_file(&entry.path(), &mut hash_hex[..], true)?;
                    println!("created: {:0o6} {}", mode, String::from_utf8_lossy(&hash_hex[..40]));
                    let tree_entry = TreeEntry::builder()
                        .path(entry.path().to_string_lossy().into_owned())
                        .mode(mode)
                        .hash(o.hash_raw.unwrap());
                }
            }
        }
        Ok(())
    }
}

