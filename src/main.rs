use codecrafters_git::cli::Git;

fn main() {
    let mut git = match Git::from_env() {
        Ok(git) => git,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        },
    };

    match git.run() {
        Err(e) => eprintln!("{}", e),
        Ok(_) => (),
    };

}
