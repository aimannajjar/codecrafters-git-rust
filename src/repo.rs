use std::{fmt::Display, fs, io::Write};
use winnow::{
    Parser,
    combinator::alt,
    token::{self},
};

use crate::{GitError, GitResult};

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

impl <'a> RefComputeRequest<'a> {

}

impl<'a> RefDiscoveryRequest<'a> {
    fn from_client(mut client: GitClient<'a>) -> Self {
        RefDiscoveryRequest {
            client
        }
    }

    async fn exec(mut self) -> GitResult<RefComputeRequest<'a>> {
        let client = reqwest::Client::new();
        let url = git_url!(self.client.url, "git-upload-pack");
        let resp = client.get(url).send().await.map_err(GitError::HttpError)?;
        let resp = resp.text().await.unwrap();
        let mut compute = RefComputeRequest {
            client: self.client,
            advertised: Vec::new(),
            common: Vec::new(),
        };
        for mut l in resp.lines().skip(1) {
            let Ok(r) = parse_ref_line.parse_next(&mut l) else {
                break;
            };
            compute.advertised.push(r);
        }
        Ok(compute)
    }

    
}

impl<'a> GitClient<'a> {
    fn repo(url: &'a str) -> Self {
        Self { url }
    }

    fn discovery(mut self) -> RefDiscoveryRequest<'a> {
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
        hash: hash.to_string(),
        name: name.to_string(),
    })
}

fn parse_pkt_len<'s>(line: &mut &'s str) -> winnow::Result<&'s str> {
    token::take_while(1.., ('0'..='9', 'a'..='f', 'A'..='F')).parse_next(line)
}
