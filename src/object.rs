use crate::{GitError, GitResult, tree::Tree};
use const_hex;
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use sha1_checked::{Digest, Sha1};

use std::{
    fmt::Display,
    fs::{self, File},
    io::{self, BufRead, BufReader, Cursor, Read, Take, Write},
    path::PathBuf,
    time, usize,
};

#[derive(Debug, PartialEq)]
pub(crate) enum ObjectType {
    Blob,
    Tree,
}

impl Display for ObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let t = match self {
            Self::Blob => "blob",
            Self::Tree => "tree",
        };
        write!(f, "{}", t)
    }
}

impl ObjectType {
    fn from_bytes<'a>(bytes: &[u8]) -> GitResult<ObjectType> {
        match bytes {
            b"blob" => Ok(ObjectType::Blob),
            b"tree" => Ok(ObjectType::Tree),
            _ => Err(GitError::ObjectError("invalid object type".to_string())),
        }
    }

    fn to_bytes(&self) -> &'static [u8] {
        match self {
            ObjectType::Blob => b"blob",
            ObjectType::Tree => b"tree",
        }
    }
}

const MAX_OBJECT_HEADER: usize = 32;

pub struct Object<R: Read> {
    reader: Option<BufReader<R>>, // should be accessied through reader()
    pub(crate) hash: Option<String>,
    pub(crate) size: Option<usize>,
    pub(crate) object_type: Option<ObjectType>,
}

impl<R: Read> Object<R> {
    /// Parse an existing object from a buffer
    /// This attempts to parse the object header and then stops.
    /// To read object content, use reader() 
    /// This does NOT populate hash field
    pub(crate) fn from_buffer(reader: R) -> GitResult<Self> {
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

        Ok(Object { hash: None, reader: Some(reader), size: Some(object_size), object_type: Some(object_type) })
    }

    /// Returns a BufReader that can be used to read the decompressed object file contents  
    pub(crate) fn reader(self) -> GitResult<Take<BufReader<R>>> {
        assert!(!self.size.is_none());
        let size = self.size.unwrap() as u64;
        match self.reader {
            Some(reader) => Ok(reader.take(size)),
            None => Err(GitError::ObjectError("object file is not on disk".to_string())),
        }
    }

    /// basic sha1 validation
    fn validate_object_hash(object: &str) -> GitResult<()> {
        if object.len() < 40 {
            return Err(GitError::ObjectError(format!("invalid object hash provided: {}", object)))
        }
        Ok(())
    }

    pub(crate) fn cat_object<O: io::Write>(reader: R, mut out: O) -> GitResult<()> {
        let obj = Object::from_buffer(reader)?;
        let mut reader = obj.reader()?;
        io::copy(&mut reader, &mut out).map_err(GitError::IOError)?;
        Ok(())
    }

    pub(crate) fn ls_tree<O: io::Write>(reader: R, mut out: O, flags: u8) -> GitResult<()> {
        let obj = Object::from_buffer(reader)?;
        match obj.object_type.as_ref() {
            Some(t) if *t != ObjectType::Tree => Err(GitError::ObjectError(format!("object not a tree: {}", *t))),
            None => Err(GitError::ObjectError("unknown object type".to_string())),
            _ => Ok(())
        }?;
        Tree::show_tree(obj, &mut out, flags)?;
        Ok(())
    }
}


/// Implementation for objects stored as compressed files
impl Object<File> {
    // Given any raw file, create a git object of type blob
    // This will compute and populate the hash field
    // If write is set, the object will be persisted on disk upon creation
    pub(crate) fn create_from_file(reader: File, do_write: bool) -> GitResult<Self> {
        let mut reader = BufReader::new(reader);
        let mut hasher = Sha1::new();
        let mut path: Option<PathBuf> = None;

        let mut disk_writer = if do_write {
            // create .git/objects/timestamp, will later rename to actual file once hash is
            let tmpname = time::SystemTime::now()
                .duration_since(time::UNIX_EPOCH)
                .map_err(|e| GitError::ObjectError(format!("error creating tmp file: {}", e)))?
                .as_secs()
                .to_string();

            path.replace(PathBuf::from(".git/objects").join(tmpname));
            fs::create_dir_all(path.as_ref().unwrap().parent().unwrap()).map_err(GitError::IOError)?;
            let f = File::create(path.as_ref().unwrap()).map_err(GitError::IOError)?;
            Some(ZlibEncoder::new(f, Compression::fast()))
        } else {
            None
        };

        fn write(w: &mut Option<ZlibEncoder<File>>, b: &[u8], h: &mut Sha1) -> GitResult<()> {
            if let Some(w) = w {
                w.write_all(b).map_err(GitError::IOError)?;
            }
            h.update(b);
            Ok(())
        }

        let size = reader.get_ref().metadata().map_err(GitError::IOError)?.len() as usize;

        // format size as str
        let mut sizestr = [0u8; 64];
        let mut c = Cursor::new(&mut sizestr[..]);
        write!(c, "{}", size).map_err(GitError::IOError)?;
        let slen = c.position() as usize;

        // write header
        write(&mut disk_writer, ObjectType::Blob.to_bytes(), &mut hasher)?;
        write(&mut disk_writer, b" ", &mut hasher)?;
        write(&mut disk_writer, &sizestr[..slen], &mut hasher)?;
        write(&mut disk_writer, b"\0", &mut hasher)?;

        // write body
        let mut written = 0;
        loop {
            let mut buf = [0u8; 8*1024];
            let n = reader.read(&mut buf).map_err(GitError::IOError)?;
            written = written + n;
            if n == 0 { break }
            write(&mut disk_writer, &buf[..n], &mut hasher)?;
        }
        assert_eq!(written, size);

        // hex encode sha1
        let hash = const_hex::encode(hasher.finalize());

        // flush write to disk and rename based on sha
        let mut reader = None;
        if let Some(mut w) = disk_writer {
            w.flush().map_err(GitError::IOError)?;
            let old_path = path.unwrap();
            let path = PathBuf::from(".git/objects")
                .join(&hash[0..2])
                .join(&hash[2..]);
            fs::create_dir_all(path.parent().unwrap()).map_err(GitError::IOError)?;
            fs::rename(old_path, &path).map_err(GitError::IOError)?;
            reader.replace(BufReader::new(File::open(path).map_err(GitError::IOError)?));
        }

        Ok(Object {
            reader,
            object_type: Some(ObjectType::Blob),
            hash: Some(hash),
            size: Some(size),
        })
    }

    /// Given an object hash, attempt to read it from filesystem, parse it and print its content to out
    pub(crate) fn cat_object_from_hash<O: io::Write>(hash: &str, mut out: O) -> GitResult<()> {
        Self::validate_object_hash(hash)?;
        let path = PathBuf::from(".git/objects").join(&hash[0..2]).join(&hash[2..]);
        let f = File::open(path).map_err(GitError::IOError)?;
        let decoder = ZlibDecoder::new(f);
        Object::cat_object(decoder, &mut out)
    }

    /// Given an tree object hash, attempt to read it from filesystem, parse it and print its content to out
    pub(crate) fn ls_tree_from_hash<O: io::Write>(hash: &str, mut out: O, flags: u8) -> GitResult<()> {
        Self::validate_object_hash(hash)?;
        let path = PathBuf::from(".git/objects").join(&hash[0..2]).join(&hash[2..]);
        let f = File::open(path).map_err(GitError::IOError)?;
        let decoder = ZlibDecoder::new(f);
        Object::ls_tree(decoder, &mut out, flags)
    }

    pub(crate) fn hash_object_from_file<O: io::Write>(
        path: &PathBuf,
        mut out: O,
        write: bool,
    ) -> GitResult<()> {
        let f = File::open(path).map_err(GitError::IOError)?;
        let o = Object::create_from_file(f, write)?;
        writeln!(&mut out, "{}", o.hash.as_ref().unwrap()).map_err(GitError::IOError)?;
        Ok(())
    }
}

