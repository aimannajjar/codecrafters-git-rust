use std::io::{Cursor, Write};

use crate::{GitError, GitResult, gwrite, gwriteln, object::{Object, ObjectType}};

pub(crate) struct Commit;

impl Commit {
    pub(crate) fn create_commit<O: Write>(mut out: O, tree: &str, parent: Option<&str>, author: &str, message: &str) -> GitResult<()> {
        let mut buf = Vec::new();
        gwrite!(buf, "tree {}\n", tree)?;
        if let Some(parent) = parent {
            gwrite!(buf, "parent {}\n", parent)?;
        }
        gwrite!(buf, "author {}\n", author)?;
        gwriteln!(buf, "committer {}\n", author)?;
        gwriteln!(buf, "{}", message)?;

        let size = buf.len();
        let mut buf = Cursor::new(buf);
        let o = Object::create_from_buffer(&mut buf, ObjectType::Commit, size, true)?;
        gwriteln!(out, "{}", o.hash_hex.expect("created object is not hashed"))
    }

}
