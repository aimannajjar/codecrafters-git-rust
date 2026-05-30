use std::{fs, io::{self, BufRead, BufReader, Cursor, Read}, path::PathBuf};
use flate2::{read::ZlibDecoder};

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

macro_rules! string_to_type {
    ($t:ident) => {
        match $t.as_str() {
            "blob" => ObjectType::Blob,
            _ => ObjectType::Invalid,
        }
    }
}

type GitResult<T> = Result<T, GitError>;

#[derive(Debug)]
enum GitError {
    IOError(io::Error),
    ObjectError(String),
}

#[derive(Debug, PartialEq)]
enum ObjectType {
    Blob,
    Invalid,
}

pub struct Git<O, E> {
    pub err: E,
    pub out: O,
}

const MAX_OBJECT_HEADER: usize = 32;

struct Object {
    reader: BufReader<ZlibDecoder<Cursor<Vec<u8>>>>,
    object_type: ObjectType,
    object_size: usize,
}

impl Object {
    fn try_from(reader: ZlibDecoder<Cursor<Vec<u8>>>) -> GitResult<Self> {
        let mut reader = BufReader::new(reader);
        let mut buf = Vec::with_capacity(MAX_OBJECT_HEADER);

        // Parse type
        let tlen = reader
            .by_ref()
            .take(MAX_OBJECT_HEADER as u64)
            .read_until(b' ', &mut buf)
            .map_err(GitError::IOError)?;

        if buf.ends_with(&[b' ']) {
            buf.pop();
        }
        let object_type = String::from_utf8(buf.clone()).map_err(|_| GitError::ObjectError(format!("could not parse object type")))?;
        let object_type = string_to_type!(object_type);
        if object_type == ObjectType::Invalid {
            return Err(GitError::ObjectError(format!("invalid object type")))
        }

        // Parse size
        buf.clear();
        reader
            .by_ref()
            .take((MAX_OBJECT_HEADER - tlen) as u64)
            .read_until(b'\0', &mut buf)
            .map_err(GitError::IOError)?;

        if !buf.ends_with(&[b'\0']) {
            return Err(GitError::ObjectError(format!("invalid header")))
        }
        buf.pop();
        let object_size: usize = String::from_utf8(buf)
            .map_err(|_| GitError::ObjectError(format!("could not parse object size")))?
            .parse()
            .map_err(|_| GitError::ObjectError(format!("could not parse object size")))?;

        Ok(Object { object_type, object_size, reader })
    }

    fn reader(self) -> BufReader<ZlibDecoder<Cursor<Vec<u8>>>> {
        self.reader
    }

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

    fn validate_object_hash(&self, object: &str) -> GitResult<()> {
        if object.len() < 40 {
            return Err(GitError::ObjectError(format!("Invalid object hash provided: {}", object)))
        }
        Ok(())
    }

    pub fn cat_file(&mut self, object: &str) {
        match self.do_cat_file(object) {
            Err(GitError::IOError(e)) => eprintln!("Error: {}", e.to_string()),
            Err(GitError::ObjectError(e)) => eprintln!("{}", e),
            Ok(_) => (),
        }
    }

    fn do_cat_file(&mut self, object: &str) -> GitResult<()> {
        self.validate_object_hash(object)?;
        let path = PathBuf::from(".git/objects").join(&object[0..2]).join(&object[2..]);
        let f = fs::read(path).map_err(GitError::IOError)?;
        self.do_cat_object(f)
    }

    fn do_cat_object(&mut self, bytes: Vec<u8>) -> GitResult<()> {
        let cursor = Cursor::new(bytes);
        let decoder = ZlibDecoder::new(cursor);
        let obj = Object::try_from(decoder)?;
        
        io::copy(&mut obj.reader(), &mut self.out).map_err(GitError::IOError)?;
        
        Ok(())
    }


}
