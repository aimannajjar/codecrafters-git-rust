use std::{io, process};
use codecrafters_git::{Git, cli::{GitOptions, GitCommand}};

fn main() {
    let options = match GitOptions::try_from_env() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        },
    };

    let mut git = Git {
        out: io::stdout(),
        err: io::stderr(),
    };

    match options.command {
        GitCommand::Init => git.init(),
        GitCommand::CatFile => {
            if options.hash == "" {
                eprintln!("object is required, example git cat-file -p 242c034c1201555d8c05e812417e0a527afb35a7");
                process::exit(1);
            }
            git.cat_file(&options.hash)
        },
        GitCommand::Unset => todo!() // implement usage
    };
}
