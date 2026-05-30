use codecrafters_git::{GitError, cli::Git};

fn main() {
    let mut git = match Git::from_env() {
        Ok(git) => git,
        Err(GitError::CLIError(e)) => {
            eprintln!("{}", e);
            std::process::exit(1);
        },
        Err(e) => panic!("unexpected error: {:?}", e),
    };

    match git.run() {
        Err(GitError::IOError(e)) if e.kind() == std::io::ErrorKind::BrokenPipe => {},
        Err(GitError::IOError(e)) => {
            eprintln!("i/o error: {}", e.to_string());
        },
        Err(GitError::ObjectError(e)) => eprintln!("{}", e),
        Err(e) => panic!("unexpected error: {:?}", e),
        Ok(_) => (),
    };

}
