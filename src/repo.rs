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
        // body.push_str("0000\n");
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

        println!(
            "RAW BODY:\n--------------------\n{}\n-----------------\n",
            resp_text
        );

        let mut pack_data: Vec<u8> = Vec::new();
        while let Some(pkt) = parse_pkt_line.parse_next(&mut stream).expect("parse error") {
            let mut pkt_stream = pkt;
            if let Ok(Some(pkt)) = parse_pack.parse_next(&mut pkt_stream) {
                println!("pack data: {:#?}", pkt);
                pack_data.extend(pkt);
            } else {
                println!("ignoring non pack data");
            }
            // println!("pkt is {}", String::from_utf8_lossy(pkt));
        }

        println!("-------------------------");
        println!("full pack data: {:?}", String::from_utf8_lossy(&pack_data));
        println!("-------------------------");
        let mut z = ZlibDecoder::new(&*pack_data.as_mut_slice());
        let mut pack_body_decoded = Vec::new();
        z.read_to_end(&mut pack_body_decoded);
        println!("pack_body: {:?}", pack_body_decoded);
        println!("-------------------------");
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

fn parse_pack<'s>(input: &mut &'s [u8]) -> winnow::Result<Option<&'s [u8]>> {
    let designator = token::take(1usize).parse_next(input)?;
    if designator[0] != 1 {
        return Ok(None);
    }
    let start = input.checkpoint();
    if let Ok(header) = token::literal::<_,_,ContextError>(b"PACK").parse_next(input) {
        println!("found header");
        let version = be_u32.parse_next(input)?;
        let objects_count = be_u32.parse_next(input)?;
        println!("objects count: {:?}", objects_count);

        let object_type =
            bits::<_, u8, ContextError, _, _>(bits::take(3usize)).parse_next(input)?;
        println!("first object type is {:?}", object_type);

        bits::<_, u8, ContextError, _, _>(bits::take(7usize)).parse_next(input)?;

        let object_len = bits::<_, u8, ContextError, _, _>(bits::take(4usize)).parse_next(input)?;
        println!("first object len is {:?}", object_len);

        let pack_body = token::take(object_len).parse_next(input)?;
        return Ok(Some(pack_body))
    } 
    input.reset(&start);
    let pack_body = token::rest.parse_next(input)?;
    // println!("pack_body compresssed: {:?}", String::from_utf8_lossy(pack_body));
    // let mut z = ZlibDecoder::new(pack_body);
    // let mut pack_body_decoded = Vec::new();
    // z.read_to_end(&mut pack_body_decoded);
    // println!("pack_body: {:?}", pack_body_decoded);
    Ok(Some(pack_body))
}

fn parse_pkt_line<'s>(input: &mut &'s [u8]) -> winnow::Result<Option<&'s [u8]>> {
    let pkt_len = token::take(4usize).parse_next(input)?;
    if pkt_len == b"0000" {
        return Ok(None);
    }
    let pkt_len =
        u32::from_str_radix(&String::from_utf8_lossy(pkt_len), 16).expect("invalid pkt len");

    // let start = input.checkpoint();
    // let band = bits::<_, u8, ContextError, _, _>(bits::take(1usize)).parse_next(input)?;
    // let band: &[u8] = &[band];
    // let mut s = band;
    // if let Ok(band) = alt::<_, _, ContextError, _>((b"\x01", b"\x02", b"\x03")).parse_next(&mut s) {
    //     println!("found band designator")
    // } else {
    //     input.reset(&start);
    // }
    // // let pkt_len: u32 = String::from_utf8_lossy(pkt_len).parse().expect("failed to parse pkt len");
    Ok(Some(token::take(pkt_len - 4).parse_next(input)?))
}

fn parse_upload_pack_respone<'s>(input: &mut &'s [u8]) -> winnow::Result<&'s [u8]> {
    digit1.parse_next(input)?;
    let ack = alt((b"NAK", b"ACK")).parse_next(input)?;
    if ack == b"NAK" {
        // extract pack
        token::take_until(0.., &b"PACK"[..]).parse_next(input)?;
        token::literal(b"PACK").parse_next(input)?;
        let mut pack = token::rest.parse_next(input)?;
        parse_pack.parse_next(&mut pack)?;
        Ok(pack)
    } else {
        Ok(ack)
    }
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
