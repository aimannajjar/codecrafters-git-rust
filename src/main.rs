use codecrafters_git::cli::{Git, GitCommand};

#[tokio::main]
async fn main() {
    let mut out = Vec::new();
    let cmd = GitCommand::Clone { url: "http://192.168.80.71:3001/codecrafters/test-repo.git".to_string() };
    let git = Git::with_bytes_buffer(cmd, &mut out);
    git.run().await.expect("failed");
    println!("{}", String::from_utf8_lossy(&out));

    // let git = match Git::from_env() {
    //     Ok(git) => git,
    //     Err(e) => {
    //         eprintln!("{}", e);
    //         std::process::exit(1);
    //     }
    // };
    //
    // if let Err(e) = git.run().await {
    //     match e {
    //         GitError::IOError(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
    //         _ => {
    //             eprintln!("{}", e);
    //             std::process::exit(1);
    //         }
    //     };
    // }
}
