use crate::{GitError, GitResult};
use const_hex;
use flate2::{Compression, bufread::ZlibEncoder, read::ZlibDecoder};
use sha1_checked::{Digest, Sha1};

use std::{
    fs::{DirBuilder, File},
    io::{self, BufRead, BufReader, Cursor, Read, Write},
    path::PathBuf, usize,
};

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

    fn to_bytes(&self) -> &'static [u8] {
        match self {
            ObjectType::Blob => b"blob",
        }
    }
}

const MAX_OBJECT_HEADER: usize = 32;

pub struct Object<R: Read> {
    data: BufReader<R>,
    hash: Option<String>,
}

impl<R: Read> Object<R> {
    /// Parse an existing object from a buffer
    /// This attempts to parse the object header and then stops.
    /// To read object content, use reader() 
    /// This does NOT populate hash field
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
        let _object_type = ObjectType::from_bytes(buf.as_slice())?;

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
        let _object_size: usize = String::from_utf8(buf)
            .map_err(|_| GitError::ObjectError(format!("could not parse object size")))?
            .parse()
            .map_err(|_| GitError::ObjectError(format!("could not parse object size")))?;

        Ok(Object { hash: None, data: reader })
    }

    /// Returns a BufReader that can be used to read the decompressed object file contents  
    fn reader(self) -> BufReader<R> {
        self.data
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


/// Implementation for new objects represented in memory
impl Object<Cursor<Vec<u8>>> {
    // Given a raw buffer, create an object
    // This will compute and populate the hash field
    pub(crate) fn from_raw_file(reader: File) -> GitResult<Self> {
        let mut reader = BufReader::new(reader);
        let mut objbuf: Vec<u8> = Vec::new();
        let size = reader.get_ref().metadata().map_err(GitError::IOError)?.len();

        // format size as str
        let mut sizestr = [0u8; 64];
        let mut c = Cursor::new(&mut sizestr[..]);
        write!(c, "{}", size).map_err(GitError::IOError)?;
        let slen = c.position() as usize;

        // write header
        objbuf.extend_from_slice(ObjectType::Blob.to_bytes());
        objbuf.push(b' ');
        objbuf.extend_from_slice(&sizestr[..slen]);
        objbuf.push(b'\0');

        // write body
        reader.read_to_end(&mut objbuf).map_err(GitError::IOError)?;

        // hash
        let sha1 = const_hex::encode(Sha1::digest(&objbuf));
        Ok(Object {
            data: BufReader::new(Cursor::new(objbuf)),
            hash: Some(sha1),
        })
    }

    /// Persists object on disk
    fn save_to_disk(self) -> GitResult<()> {
        assert!(!self.hash.is_none()); // shouldn't happen
        let hash = self.hash.as_ref().unwrap();
        let path = PathBuf::from(".git/objects")
            .join(&hash[0..2])
            .join(&hash[2..]);

        // create .git/objects/xy were xy is first two digits of hash
        DirBuilder::new()
            .recursive(true)
            .create(path.parent().unwrap())
            .map_err(GitError::IOError)?;

        // prepare reader and writer
        let mut f = File::create(&path).map_err(GitError::IOError)?;
        let mut encoder = ZlibEncoder::new(self.reader(), Compression::fast());
        io::copy(&mut encoder, &mut f).map_err(GitError::IOError)?;
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

    pub(crate) fn hash_object_from_file<O: io::Write>(
        path: &PathBuf,
        mut out: O,
        write: bool,
    ) -> GitResult<()> {
        let f = File::open(path).map_err(GitError::IOError)?;
        let o = Object::from_raw_file(f)?;
        writeln!(&mut out, "{}", o.hash.as_ref().unwrap()).map_err(GitError::IOError)?;
        if write {
            o.save_to_disk()?;
        }
        Ok(())
    }
}

