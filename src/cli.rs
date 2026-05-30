use std::env;


// Represents a parsable CLI arg
// instanstiate one for each valid flag or action in CLI_ARGS
struct CliArg {
    name: &'static str,
    short: char,
    on_set: fn(&mut GitOptions) -> Result<(), String>,
}


// Instances of valid CliArg 
const CLI_ARGS: &'static [CliArg] = &[
    CliArg {
        name: "init",
        short: '\0',
        on_set: |options| {
            options.command = GitCommand::Init;
            Ok(())
        }
    },

    CliArg {
        name: "cat-file",
        short: '\0',
        on_set: |options| {
            options.command = GitCommand::CatFile;
            Ok(())
        }
    },

    CliArg {
        name: "pretty-print",
        short: 'p',
        on_set: |options| {
            match options.command {
                GitCommand::CatFile => {
                    options.pretty_print = true;
                    Ok(())
                },
                _ => {
                    Err("invalid flag -p".to_string())
                }
            }
        }
    },
];

#[derive(Debug, PartialEq)]
pub enum GitCommand {
    Unset,
    Init,
    CatFile,
}

#[derive(Debug)]
pub struct GitOptions {
    pub command: GitCommand,
    pub hash: String,
    pub pretty_print: bool,
}

impl Default for GitOptions {
    fn default() -> Self {
        Self {
            command: GitCommand::Unset,
            hash: String::with_capacity(40),
            pretty_print: false,
        }
    }
}

impl GitOptions {
    pub fn try_from_env() -> Result<Self, String> {
        let mut args = env::args();
        let _ = args.next().unwrap(); // binary name
        let mut options = GitOptions::default();
        
        while let Some(arg) = args.next() {
            if arg.starts_with("-") {
                // ---------------------
                // - short form flag(s)
                if options.command == GitCommand::Unset {
                    return Err("please specify command first then flags".to_string())
                }

                // process all chars in flag group
                let mut chars = arg.chars();
                let _ = chars.next().unwrap(); // remove that dash
                while let Some(c) = chars.next() {
                    if c == ' ' {
                        break;
                    }
                    let _= CLI_ARGS
                        .iter()
                        .filter(|a| a.short == c)
                        .try_for_each(|o| -> Result<(), String> { (o.on_set)(&mut options) })?;
                }
            } else if options.command == GitCommand::Unset {
                // ---------------------
                // positional arg where command has yet to be set, validate command
                let _= match CLI_ARGS.iter().filter(|a| a.name == arg).next() {
                    Some(arg) => (arg.on_set)(&mut options),
                    _ => Err(format!("Invalid command: {}", arg)),
                }?;
            } else {
                // -----------------------
                // another positional arg
                options.take_argument(&arg)?;
            }
        }
        Ok(options)
    }

    fn take_argument(&mut self, arg: &str) -> Result<(), String> {
        match &self.command {
            GitCommand::CatFile if self.hash == "" => {
                self.hash.push_str(&arg);
                Ok(())
            }
            _ => Err(format!("unexpected positional argument: {}", arg)),
        }
    }
}

