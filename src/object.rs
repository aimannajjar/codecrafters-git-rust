use std::{fs, io::{self, BufRead, BufReader, Cursor, Read}, path::PathBuf};
use flate2::{read::ZlibDecoder};
use crate::{GitResult, GitError};

macro_rules! string_to_type {
    ($t:ident) => {
        match $t.as_str() {
            "blob" => ObjectType::Blob,
            _ => ObjectType::Invalid,
        }
    }
}

#[derive(Debug, PartialEq)]
enum ObjectType {
    Blob,
    Invalid,
}

const MAX_OBJECT_HEADER: usize = 32;

pub struct Object {
    reader: BufReader<ZlibDecoder<Cursor<Vec<u8>>>>,
    _object_type: ObjectType,
    _object_size: usize,
}

impl Object {
    /// Creates an object by scanning a the header for type and size.
    /// This attempts to parse the object header and then stops.
    /// To read object content, use reader() 
    pub(crate) fn from(reader: ZlibDecoder<Cursor<Vec<u8>>>) -> GitResult<Self> {
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
        let object_type = String::from_utf8(buf.clone())
            .map_err(|_| GitError::ObjectError(format!("could not parse object type")))?;
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

        Ok(Object { _object_type: object_type, _object_size: object_size, reader })
    }

    /// Returns a BufReader that can be used to read the decompressed object file contents  
    fn reader(self) -> BufReader<ZlibDecoder<Cursor<Vec<u8>>>> {
        self.reader
    }

    /// basic sha1 validation
    fn validate_object_hash(object: &str) -> GitResult<()> {
        if object.len() < 40 {
            return Err(GitError::ObjectError(format!("Invalid object hash provided: {}", object)))
        }
        Ok(())
    }

    /// Given an object hash, attempt to read it, parse it and print its content to out
    pub(crate) fn cat_object_file<O: io::Write>(hash: &str, mut out: O) -> GitResult<()> {
        Self::validate_object_hash(hash)?;
        let path = PathBuf::from(".git/objects").join(&hash[0..2]).join(&hash[2..]);
        let f = fs::read(path).map_err(GitError::IOError)?;
        let decoder = ZlibDecoder::new(Cursor::new(f));
        let obj = Object::from(decoder)?;
        io::copy(&mut obj.reader(), &mut out).map_err(GitError::IOError)?;
        Ok(())
    }


}
