use std::{
    ffi::OsStr,
    fmt::Display,
    fs::{self, DirEntry},
    io::{BufRead, BufReader, BufWriter, Cursor, Read, Take, Write},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
};

use crate::{
    GitError, GitResult, gwrite, gwriteln,
    object::{Object, ObjectType},
};

#[allow(non_camel_case_types)]
type mode_t = u32;

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
        let size = 20 + 2 + self.path.as_ref().unwrap().len() + format!("{:o}", self.mode).len();
        TreeEntry {
            mode: self.mode,
            path: self.path.unwrap(),
            hash: self.hash,
            size,
        }
    }
}

impl Default for TreeEntryBuilder {
    fn default() -> Self {
        Self {
            mode: 0,
            path: None,
            hash: [0u8; 20],
        }
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
        name_only: bool,
    ) -> GitResult<()> {
        let mut tree_size = object.size.unwrap(); // shouldn't panic
        let mut reader = object.reader()?;
        while tree_size > 0 {
            let tree_entry = Self::read_tree_entry(&mut reader)?;
            tree_size = tree_size - tree_entry.size;
            if name_only {
                gwriteln!(&mut out, "{}", tree_entry)?;
            } else {
                let hash = const_hex::encode(&tree_entry.hash);
                gwriteln!(&mut out, "{:06o} {}\t{}", tree_entry.mode, hash, tree_entry)?;
                if let Ok(f) = fs::File::open(format!(".git/objects/{}", hash)) {
                    let ot = Object::from_buffer(f)?;
                    let ot = ot.object_type.expect("object instantiated succesfully should have valid type");
                    gwriteln!(&mut out, "{:06o} {} {}\t{}", tree_entry.mode, ot, hash, tree_entry)?;
                }
            }
        }
        if tree_size != 0 {
            Err(GitError::ObjectError(
                "tree file size entries mistmach".to_string(),
            ))
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
                break;
            } else if c[0] < b'0' || c[0] > b'7' {
                return Err(GitError::ObjectError(format!(
                    "tree object malformed, unexpected byte: {}",
                    c[0]
                )));
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
        Ok(TreeEntry {
            mode,
            hash,
            path,
            size,
        })
    }

    pub(crate) fn write_tree(dir: PathBuf) -> GitResult<[u8; 20]> {
        let mut buf: Vec<u8> = Vec::new();
        let writer = BufWriter::new(&mut buf);
        let size = Self::write_tree_recursive(writer, dir)?;
        let o = Object::create_from_buffer(Cursor::new(buf), ObjectType::Tree, size, true)?;
        Ok(o.hash_raw
            .expect("created from buffer object was not hashed"))
    }

    fn write_tree_recursive<W: Write>(mut writer: BufWriter<W>, dir: PathBuf) -> GitResult<usize> {
        let mut size = 0;
        let mut entries: Vec<DirEntry> = fs::read_dir(&dir)
            .map_err(GitError::IOError)?
            .filter_map(Result::ok)
            .filter(|e| !e.file_name().to_string_lossy().starts_with("."))
            .filter(|e| e.file_name().to_string_lossy() != "target")
            .collect();

        entries.sort_by(|a, b| a.path().file_name().cmp(&b.path().file_name()));

        for entry in entries {
            let tree_entry;
            let path = entry.path();
            let rel_path = path
                .strip_prefix(&dir)
                .map_err(|_| GitError::ObjectError("path not child of current dir".to_string()))?;
            if entry.path().file_name() == Some(OsStr::new(".git")) {
                continue;
            } else if entry.path().is_dir() {
                let hash = Self::write_tree(entry.path())?;
                tree_entry = Some(
                    TreeEntry::builder()
                        .path(rel_path.to_string_lossy().into_owned())
                        .mode(0o40000)
                        .hash(hash)
                        .build(),
                );
            } else {
                let mut hash_hex = [0u8; 41];
                let metadata = entry.path().metadata().map_err(GitError::IOError)?;
                let mode = metadata.permissions().mode();
                let o = Object::hash_object_from_file(entry.path(), &mut hash_hex[..], true)?;

                tree_entry = Some(
                    TreeEntry::builder()
                        .path(rel_path.to_string_lossy().into_owned())
                        .mode(mode)
                        .hash(o.hash_raw.unwrap())
                        .build(),
                );
            }

            if let Some(tree_entry) = tree_entry {
                gwrite!(&mut writer, "{:o} ", tree_entry.mode)?;
                gwrite!(&mut writer, "{}", tree_entry.path)?;
                writer.write(b"\0").map_err(GitError::IOError)?;
                writer.write(&tree_entry.hash).map_err(GitError::IOError)?;
                size = size + tree_entry.size;
            }
        }
        Ok(size)
    }
}
