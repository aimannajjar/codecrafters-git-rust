use std::{env, io, process};
use codecrafters_git::Git;

fn main() {
    let mut args = env::args();
    let _ = args.next().unwrap();
    let command = args.next().unwrap_or_else(|| { 
        eprintln!("subcommand is required, example git init");
        std::process::exit(1);
    });

    let mut git = Git {
        out: io::stdout(),
        err: io::stderr(),
    };


    if command == "init" {
        git.init();
    } else if command == "cat-file" {
        let object = args.next().unwrap_or_else(|| { 
            eprintln!("object is required, example git cat-file -p 242c034c1201555d8c05e812417e0a527afb35a7");
            process::exit(1);
        });

        git.cat_file(&object);
    } else {
        println!("unknown command: {}", command);
    }
}
