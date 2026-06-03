use codecrafters_git::{GitError, cli::Git};

fn main() {
    let git = match Git::from_env() {
        Ok(git) => git,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = git.run() {
        match e {
            GitError::IOError(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
            _ => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        };
    }
}
