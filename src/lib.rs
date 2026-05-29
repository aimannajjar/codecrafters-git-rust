use std::{fs, io::{self, Read}, path::PathBuf};
use flate2::{read::ZlibDecoder};

type GitResult<T> = Result<T, GitError>;

#[derive(Debug)]
enum GitError {
    IOError(io::Error),
    InvalidObject(String),
}

pub struct Git<O, E> {
    pub out: O,
    pub err: E,
}

macro_rules! gprint {
    ($self:ident, $f:expr) => {
        write!($self.out, $f)
    };

    ($self:ident, $f:expr, $($v:expr),* $(,)?)  => {
        write!($self.out, $f, $($v,)*)
    };
}

macro_rules! geprint {
    ($self:ident, $f:expr) => {
        write!($self.err, $f)
    };

    ($self:ident, $f:expr, $($v:expr),* $(,)?)  => {
        write!($self.err, $f, $($v,)*)
    };
}

impl<O,E> Git<O,E>
where
    O: io::Write,
    E: io::Write,
{
    pub fn init(&mut self) {
        match self.do_init() {
            Err(GitError::IOError(e)) => geprint!(self, "Error: {}", e.to_string()),
            Err(e) => panic!("unexpected error: {e:?}"),
            Ok(_) => gprint!(self, "Initialized git directory"),
        }.unwrap();
    }

    fn do_init(&self) -> GitResult<()> {
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
        Ok(())
    }

    pub fn cat_file(&self, object: &str) {
        match self.do_cat_file(object) {
            Err(GitError::IOError(e)) => eprintln!("Error: {}", e.to_string()),
            Err(GitError::InvalidObject(e)) => eprintln!("{}", e),
            Ok(c) => println!("{}", c),
        }
    }

    fn validate_object_hash(&self, object: &str) -> GitResult<()> {
        if object.len() < 40 {
            return Err(GitError::InvalidObject(format!("Invalid object hash provided: {}", object)))
        }
        Ok(())
    }

    fn do_cat_file(&self, object: &str) -> GitResult<String> {
        self.validate_object_hash(object)?;
        let path = PathBuf::from(".git/objects").join(&object[0..2]).join(&object[2..]);
        let f = fs::read(path).map_err(GitError::IOError)?;
        self.do_cat_object(&f)
    }

    fn do_cat_object(&self, bytes: &[u8]) -> GitResult<String> {
        let mut decoder = ZlibDecoder::new(bytes);
        let mut buf = String::new();
        decoder.read_to_string(&mut buf).map_err(GitError::IOError)?;

        Ok(buf)
    }


}
