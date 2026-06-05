use flate2::bufread::ZlibDecoder;
use reqwest::{Body, Version};
use std::{
    fmt::Display,
    fs,
    io::{self, Read, Write},
};
use winnow::{
    Parser,
    ascii::{digit1, hex_digit1},
    binary::{
        be_u8, be_u32,
        bits::{self, bits},
    },
    combinator::alt,
    error::{ContextError, ErrMode},
    stream::Stream,
    token::{self, rest},
};

use crate::{GitError, GitResult, gwriteln};

pub(crate) struct Repo;

macro_rules! git_url {
    ($url:expr, $service:tt) => {
        format!(concat!("{}/info/refs?service=", $service), $url)
    };
}

impl Repo {
    /// Initializes a repo in cwd
    pub(crate) fn init() -> GitResult<Self> {
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
        Ok(Repo)
    }

    /// Clone a repo
    pub(crate) async fn clone_repo<O: Write>(mut out: O, url: &str) -> GitResult<()> {
        let client = GitClient::repo(&url).discovery();
        let resp = client.exec().await?;
        resp.exec().await;
        gwriteln!(out, "done")?;
        Ok(())
    }
}

struct GitClient<'a> {
    url: &'a str,
}

struct RefDiscoveryRequest<'a> {
    client: GitClient<'a>,
}

struct RefComputeRequest<'a> {
    client: GitClient<'a>,
    advertised: Vec<Ref>,
    common: Vec<Ref>,
}

impl<'a> RefComputeRequest<'a> {
    async fn exec(mut self) -> GitResult<RefComputeRequest<'a>> {
        println!("-------------------------");
        println!("RefComputeRequest");
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
        println!(">>>>>>>>>>>>>>>>>>>>");
        println!("{:#?}", req);
        println!("{}", body);
        let resp = client.execute(req).await.map_err(GitError::HttpError)?;
        println!("<<<<<<<<<<<<<<<<<<<<");
        println!("{:#?}", resp.status());
        println!("{:#?}", resp.headers());
        let resp_body = resp.bytes().await.map_err(GitError::HttpError)?; // Bytes
        let resp_body_clone = &resp_body.clone();
        let resp_text = String::from_utf8_lossy(resp_body_clone);
        let resp_body: &[u8] = &resp_body; // &[u8] slice
        let mut stream = resp_body; // This creates a copy of the slice pointer 

        let mut compute = RefComputeRequest {
            client: self.client,
            advertised: Vec::new(),
            common: Vec::new(),
        };

        let mut c = 0;
        let mut packdata: Vec<u8> = Vec::new();
        while let Some((band, pkt)) = parse_pkt_line.parse_next(&mut stream).expect("parse error") {
            if band == 1 {
                packdata.extend(pkt);
                c = c + 1;
            } else {
                println!("OUT ({}): {}", band, String::from_utf8_lossy(pkt));
            }
        }

        let mut packdata = packdata.as_slice();
        let objects_count = parse_pack_header
            .parse_next(&mut packdata)
            .expect("invalid band 1 pkt");

        for i in 0..objects_count {
            println!("----------- parsing object {i} ---------");
            let (objlen, objtype) = parse_pack_object_header
                .parse_next(&mut packdata)
                .expect("failed to parse pack object");
            println!(">> LENGTH: {}", objlen);
            println!(">> TYPE: {}", objtype);
            println!(">>>> BODY <<<< ");
            let mut z = ZlibDecoder::new(&mut packdata);
            let mut pack_body_decoded = Vec::new();
            z.read_to_end(&mut pack_body_decoded);
            println!("{}", String::from_utf8_lossy(&pack_body_decoded));
        }
        println!("----------------------------------------");
        println!("REMAINING BYTES IN PACK: {}", token::rest_len::<_, ContextError>(&mut packdata).unwrap());

        Ok(compute)
    }
}

impl<'a> RefDiscoveryRequest<'a> {
    fn from_client(client: GitClient<'a>) -> Self {
        RefDiscoveryRequest { client }
    }

    async fn exec(self) -> GitResult<RefComputeRequest<'a>> {
        println!("-------------------------");
        println!("RefDiscoveryRequest");
        println!(">>>>>>>>>>>>>>>>>>>>");
        let client = reqwest::Client::new();
        let url = git_url!(self.client.url, "git-upload-pack");
        let resp = client.get(url).send().await.map_err(GitError::HttpError)?;
        println!("<<<<<<<<<<<<<<<<<<<<");
        println!("{:#?}", resp.headers());
        let resp_body = resp.text().await.unwrap();
        let mut compute = RefComputeRequest {
            client: self.client,
            advertised: Vec::new(),
            common: Vec::new(),
        };
        for mut l in resp_body.lines().skip(1) {
            println!("{}", l);
            let Ok(r) = parse_ref_line.parse_next(&mut l) else {
                break;
            };
            compute.advertised.push(r);
        }
        println!("-------------------------");
        Ok(compute)
    }
}

impl<'a> GitClient<'a> {
    fn repo(url: &'a str) -> Self {
        Self { url }
    }

    fn discovery(self) -> RefDiscoveryRequest<'a> {
        RefDiscoveryRequest::from_client(self)
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

fn parse_pack_header<'s>(input: &mut &'s [u8]) -> winnow::Result<u32> {
    token::literal::<_, _, ContextError>(b"PACK").parse_next(input).expect("invalid PACK signature");
    let version = be_u32.parse_next(input)?;
    let objects_count = be_u32.parse_next(input)?;
    println!("objects count: {:?}", objects_count);
    Ok(objects_count)
}

fn parse_pack_object_header<'s>(input: &mut &'s [u8]) -> winnow::Result<(usize, u8)> {
    let obj_type_size = token::take::<_, _, ContextError>(1usize)
        .parse_next(input)
        .expect("invalid first size byte")[0];
    println!("first byte size: {:08b}", obj_type_size);
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
        println!("Received flush");
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

fn parse_pkt_len<'s>(line: &mut &'s str) -> winnow::Result<&'s str> {
    token::take_while(1.., ('0'..='9', 'a'..='f', 'A'..='F')).parse_next(line)
}
