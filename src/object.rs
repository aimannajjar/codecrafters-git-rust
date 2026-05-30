use std::{fs::File, io::{self, BufRead, BufReader, Read}, path::PathBuf};
use flate2::{read::ZlibDecoder};
use crate::{GitResult, GitError};

#[derive(Debug, PartialEq)]
enum ObjectType {
    Blob,
}

impl ObjectType {
    fn from_bytes<'a>(bytes: &[u8]) -> GitResult<ObjectType> {
        match bytes {
            b"blob" => Ok(ObjectType::Blob),
            _ => Err(GitError::ObjectError("invalid object type".to_string())),
        }
    }
}

const MAX_OBJECT_HEADER: usize = 32;

pub struct Object<R: Read> {
    reader: BufReader<R>,
    _object_type: ObjectType,
    _object_size: usize,
}

impl<R: Read> Object<R> {
    /// Creates an object by scanning a the header for type and size.
    /// This attempts to parse the object header and then stops.
    /// To read object content, use reader() 
    pub(crate) fn from(reader: R) -> GitResult<Self> {
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
        let object_type = ObjectType::from_bytes(buf.as_slice())?;

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

        Ok(Object { _object_type: object_type, _object_size: object_size, reader })
    }

    /// Returns a BufReader that can be used to read the decompressed object file contents  
    fn reader(self) -> BufReader<R> {
        self.reader
    }

    /// basic sha1 validation
    fn validate_object_hash(object: &str) -> GitResult<()> {
        if object.len() < 40 {
            return Err(GitError::ObjectError(format!("Invalid object hash provided: {}", object)))
        }
        Ok(())
    }

    pub(crate) fn cat_object<O: io::Write>(reader: R, mut out: O) -> GitResult<()> {
        let obj = Object::from(reader)?;
        io::copy(&mut obj.reader(), &mut out).map_err(GitError::IOError)?;
        Ok(())
    }
}

/// Implementation for objects stored as compressed files
impl Object<File> {
    /// Given an object hash, attempt to read it from filesystem, parse it and print its content to out
    pub(crate) fn cat_object_from_hash<O: io::Write>(hash: &str, mut out: O) -> GitResult<()> {
        Self::validate_object_hash(hash)?;
        let path = PathBuf::from(".git/objects").join(&hash[0..2]).join(&hash[2..]);
        let f = File::open(path).map_err(GitError::IOError)?;
        let decoder = ZlibDecoder::new(f);
        Object::cat_object(decoder, &mut out)
    }
}
