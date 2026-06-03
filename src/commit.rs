use std::io::{Cursor, Write};

use crate::{GitError, GitResult, gwrite, gwriteln, object::{Object, ObjectType}};

struct Commit;

impl Commit {

    pub fn create_commit<O: Write>(mut out: O, tree: &str, parent: &str, author: &str, message: &str) -> GitResult<()> {
        let mut buf = Vec::new();
        gwrite!(buf, "tree {}\n", tree)?;
        gwrite!(buf, "parent {}\n", parent)?;
        gwrite!(buf, "author {}\n", author)?;
        gwriteln!(buf, "committer {}\n", author)?;
        gwrite!(buf, "{}", message)?;

        let size = buf.len();
        let mut buf = Cursor::new(buf);
        let o = Object::create_from_buffer(&mut buf, ObjectType::Commit, size, true)?;
        gwriteln!(out, "{}", o.hash_hex.expect("created object is not hashed"))
    }

}
