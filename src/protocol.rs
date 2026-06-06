use flate2::bufread::ZlibDecoder;
use reqwest::Version;
use std::{fmt::Display, io::Read, path::PathBuf};
use winnow::{Parser, binary::be_u32, combinator::alt, error::ContextError, stream::Stream, token};

use crate::{
    GitError, GitResult,
    object::{Object, ObjectType},
};

/// Follows builder-like style. Start by specifying repo using repo()
/// Then build a UploadPackDiscovery request using upload_pack()
/// which then you can execute to initiate the upload-pack sequencing
///
/// # Example
/// ```rust
/// let client = GitClient::repo("https://git.kernel.org/pub/scm/git/git.git", "./git")
///     .upload_pack()
///     .exec();
/// ````
pub(crate) struct GitClient<'a> {
    url: &'a str,
    localdir: &'a PathBuf,
}

impl<'a> GitClient<'a> {
    pub(crate) fn repo(url: &'a str, localdir: &'a PathBuf) -> Self {
        Self { url, localdir }
    }

    pub(crate) fn upload_pack(self) -> UploadPackDiscovery<'a> {
        UploadPackDiscovery { client: self }
    }
}

/// Represent the state at the first step of upload-pack req
/// Only requires URL at this point. Invoking exec will return
/// the next state of the protocol, namely compute step or UploadPackCompute
pub struct UploadPackDiscovery<'a> {
    client: GitClient<'a>,
}

pub struct UploadPackCompute<'a> {
    client: GitClient<'a>,
    advertised: Vec<Ref>,
    _common: Vec<Ref>,
}

#[derive(PartialEq)]
enum PackObjectType {
    Commit,
    Tree,
    Blob,
    Tag,
    OfsDelta,
    RefDelta,
}

impl From<&PackObjectType> for ObjectType {
    fn from(value: &PackObjectType) -> Self {
        match value {
            PackObjectType::Commit => ObjectType::Commit,
            PackObjectType::Tree => ObjectType::Tree,
            PackObjectType::Blob => ObjectType::Blob,
            _ => panic!("delta packs are not true object types"),
        }
    }
}

impl<'a> UploadPackCompute<'a> {
    /// Returns sha of latest commit
    pub(crate) async fn exec(self) -> GitResult<String> {
        let url = format!("{}/{}", self.client.url, "git-upload-pack");
        let mut body = String::new();
        for obj in self.advertised.iter().skip(1) {
            body.push_str(&format!(
                "0054want {} multi_ack side-band-64k ofs-delta\n",
                obj.hash
            ));
            body.push_str(&format!("0032want {}\n", obj.hash));
        }
        body.push_str("0000");
        body.push_str("0009done\n");

        let client = reqwest::Client::new();
        let req = client
            .post(&url)
            .version(Version::HTTP_10)
            .header("content-type", "application/x-git-upload-pack-request")
            .body(body.clone())
            .build()
            .map_err(GitError::HttpError)?;
        // println!(">>>>>>>>>>>>>>>>>>>>");
        // println!("{:#?}", req);
        // println!("Request Body: {}", body);
        let resp = client.execute(req).await.map_err(GitError::HttpError)?;
        // println!("<<<<<<<<<<<<<<<<<<<<");
        // println!("{:#?}", resp.status());
        // println!("{:#?}", resp.headers());
        let resp_body = resp.bytes().await.map_err(GitError::HttpError)?; // Bytes
        let resp_body_clone = &resp_body.clone();
        let _resp_text = String::from_utf8_lossy(resp_body_clone);
        let resp_body: &[u8] = &resp_body; // &[u8] slice
        let mut stream = resp_body; // This creates a copy of the slice pointer 

        let rootdir = PathBuf::from(self.client.localdir);
        let _compute = UploadPackCompute {
            client: self.client,
            advertised: Vec::new(),
            _common: Vec::new(),
        };

        let mut c = 0;
        let mut packdata: Vec<u8> = Vec::new();
        while let Some((band, pkt)) = parse_pkt_line.parse_next(&mut stream).expect("parse error") {
            if band == 1 {
                packdata.extend(pkt);
                c = c + 1;
            }
        }

        let mut packdata = packdata.as_slice();
        let objects_count = parse_pack_header
            .parse_next(&mut packdata)
            .expect("invalid band 1 pkt");

        let mut most_recent_commit = None;
        for i in 0..objects_count {
            // println!("----------- parsing object {i} ---------");
            let (objlen, objtype) = parse_pack_object_header
                .parse_next(&mut packdata)
                .expect("failed to parse pack object");
            // println!(">> LENGTH: {}", objlen);
            // println!(">> TYPE: {}", objtype);
            // println!(">>>> BODY <<<< ");

            let objtype = match objtype {
                1 => PackObjectType::Commit,
                2 => PackObjectType::Tree,
                3 => PackObjectType::Blob,
                4 => PackObjectType::Tag,
                6 => PackObjectType::OfsDelta,
                7 => PackObjectType::RefDelta,
                _ => panic!("bad pack object type encountered"),
            };

            let mut pack_body_decoded = Vec::new();
            if objtype != PackObjectType::OfsDelta {
                let mut z = ZlibDecoder::new(&mut packdata);
                z.read_to_end(&mut pack_body_decoded)
                    .expect("failed to inflate pack object");
            } else {
                parse_ofs_delta
                    .parse_next(&mut packdata)
                    .expect("failed ofs delta offset calculation");
                let mut z = ZlibDecoder::new(&mut packdata);
                z.read_to_end(&mut pack_body_decoded)
                    .expect("failed to inflate pack object");
            }

            match &objtype {
                PackObjectType::Blob | PackObjectType::Tree | PackObjectType::Commit => {
                    // non-deltafied objects
                    let o = Object::create_from_buffer(
                        pack_body_decoded.as_slice(),
                        (&objtype).into(),
                        objlen,
                        Some(PathBuf::from(&rootdir)),
                        true,
                    )?;
                    // println!("~~~~~~~~ created object : {}", o.hash_hex.as_ref().unwrap());

                    if objtype == PackObjectType::Commit && most_recent_commit.is_none() {
                        most_recent_commit = Some(o.hash_hex.unwrap());
                    }
                }
                _ => (),
            };
            // println!("{}", String::from_utf8_lossy(&pack_body_decoded));
        }
        // println!("----------------------------------------");
        // println!(
        //     "REMAINING BYTES IN PACK: {}",
        //     token::rest_len::<_, ContextError>(&mut packdata).unwrap()
        // );

        Ok(most_recent_commit.expect("did not receive any commits"))
    }
}

impl<'a> UploadPackDiscovery<'a> {
    /// Make ref discovery request and construct UploadPackCompute instance
    /// to be used for next step of the protocol
    pub(crate) async fn exec(self) -> GitResult<UploadPackCompute<'a>> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/info/refs?service={}",
            self.client.url, "git-upload-pack"
        );
        let resp = client.get(url).send().await.map_err(GitError::HttpError)?;
        let resp_body = resp.text().await.unwrap();
        let mut compute = UploadPackCompute {
            client: self.client,
            advertised: Vec::new(),
            _common: Vec::new(),
        };
        for mut l in resp_body.lines().skip(1) {
            let Ok(r) = parse_ref_line.parse_next(&mut l) else {
                break;
            };
            compute.advertised.push(r);
        }
        Ok(compute)
    }
}

#[derive(Debug)]
struct Ref {
    hash: String,
    name: String,
}

impl Display for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

fn parse_ofs_delta<'s>(input: &mut &'s [u8]) -> winnow::Result<u32> {
    let mut c = token::take(1usize).parse_next(input)?[0];
    let mut base_offset: usize = c as usize & 127;
    while c & 128 != 0 {
        base_offset = base_offset + 1;
        if base_offset == 0 || ((!0usize << 54) & base_offset) != 0 {
            panic!("some how this is bad")
        }
        c = token::take(1usize).parse_next(input)?[0];
    }

    Ok(0)
    // unsigned base_found = 0;
    // unsigned char *pack, c;
    // off_t base_offset;
    // unsigned lo, mid, hi;
    //
    // pack = fill(1);
    // c = *pack;
    // use(1);
    // base_offset = c & 127;
    // while (c & 128) {
    // 	base_offset += 1;
    // 	if (!base_offset || MSB(base_offset, 7))
    // 		die("offset value overflow for delta base object");
    // 	pack = fill(1);
    // 	c = *pack;
    // 	use(1);
    // 	base_offset = (base_offset << 7) + (c & 127);
    // }
    // base_offset = obj_list[nr].offset - base_offset;
    // if (base_offset <= 0 || base_offset >= obj_list[nr].offset)
    // die("offset value out of bound for delta base object");
}

fn parse_pack_header<'s>(input: &mut &'s [u8]) -> winnow::Result<u32> {
    token::literal::<_, _, ContextError>(b"PACK")
        .parse_next(input)
        .expect("invalid PACK signature");
    let _version = be_u32.parse_next(input)?;
    let objects_count = be_u32.parse_next(input)?;
    // println!("objects count: {:?}", objects_count);
    Ok(objects_count)
}

fn parse_pack_object_header<'s>(input: &mut &'s [u8]) -> winnow::Result<(usize, u8)> {
    let obj_type_size = token::take::<_, _, ContextError>(1usize)
        .parse_next(input)
        .expect("invalid first size byte")[0];
    // println!("first byte size: {:08b}", obj_type_size);
    let object_type = (obj_type_size >> 4) & 7;

    let mut object_len: usize = (obj_type_size & 15) as usize;
    let mut size_byte: usize = obj_type_size as usize;
    let mut shift = 4;
    let mut c = 0;
    while size_byte & 0x80 != 0 {
        size_byte = token::take::<_, _, ContextError>(1usize)
            .parse_next(input)
            .expect(&format!("invalid size byte #{}", c))[0] as usize;
        object_len += (size_byte & 0x7f) << shift;
        shift += 7;
        c = c + 1;
    }
    Ok((object_len, object_type))
}

/// Extracts (deisgnator, buffer) tuple wrapped in Some.
/// Deisgnator will be set to 0 if no valid band is recognized
/// in which case, this 1 byte will be included in the buffer
/// Returns None if flush pkt "0000" is recognized
fn parse_pkt_line<'s>(input: &mut &'s [u8]) -> winnow::Result<Option<(u8, &'s [u8])>> {
    let pkt_len = token::take(4usize).parse_next(input)?;
    if pkt_len == b"0000" {
        // println!("Received flush");
        return Ok(None);
    }
    let pkt_len =
        u32::from_str_radix(&String::from_utf8_lossy(pkt_len), 16).expect("invalid pkt len");

    let start = input.checkpoint();
    let mut offset = 1;
    let designator = alt::<_, _, ContextError, _>((1, 2, 3))
        .parse_next(input)
        .or_else(|_| {
            input.reset(&start);
            offset = 0;
            Ok(0)
        })?;

    let rest = token::take::<_, _, ContextError>(pkt_len - 4 - offset)
        .parse_next(input)
        .expect("bad pkt length");

    Ok(Some((designator, rest)))
}

fn parse_first_ref<'s>(input: &mut &'s str) -> winnow::Result<&'s str> {
    let (refname, _, _) = ("HEAD", '\0', token::rest).parse_next(input)?;
    Ok(refname)
}

fn parse_ref_name<'s>(input: &mut &'s str) -> winnow::Result<&'s str> {
    alt((parse_first_ref, token::rest)).parse_next(input)
}

fn parse_ref_line<'s>(line: &mut &'s str) -> winnow::Result<Ref> {
    // PKT-LINE(obj-id SP refname NUL capability-list)
    let hash = token::take_while(1.., ('0'..='9', 'a'..='f', 'A'..='F')).parse_next(line)?;
    " ".parse_next(line)?;
    let name = parse_ref_name.parse_next(line)?;

    Ok(Ref {
        hash: hash[4..].to_string(),
        name: name.to_string(),
    })
}
