use std::{env, io::{self, Stdout}, path::PathBuf};
use crate::{
    GitError, object::Object, repo::Repo
};

// Expects first parameter to contian `out` field that implements io::Write
macro_rules! gprintln {
    ($self:ident, $($tt:tt)*) => {
        writeln!($self.out, $($tt)*).map_err(GitError::IOError)
    };
}

// Trait represneting how CliArg can mutate Git instance invocation
trait GitInstance {
    fn set_command(&mut self, cmd: GitCommand);
    fn set_pretty_print(&mut self, pp: bool);
    fn set_write_object(&mut self, write_object: bool);
    fn command(&mut self)  -> &GitCommand;
}

// Represents a parsable CLI arg (command, flags)
// instanstiate one for each valid flag or command in CLI_ARGS
// don't use those for positional arguments that are passed to subcommands, 
// those are processed in take_argument
struct CliArg {
    name: &'static str,
    short: char,
    // TODO: Make this static
    on_set: fn(&mut dyn GitInstance) -> Result<(), GitError>,
}

// Instances of valid CliArg 
// on_set should validate that it's being used on an appropriate command
// e.g. if -p (pretty-print) is called with wrong command, it should return Err
const VALID_CLI_ARGS: &'static [CliArg] = &[
    CliArg {
        name: "init",
        short: '\0',
        on_set: |git| {
            git.set_command(GitCommand::Init);
            Ok(())
        }
    },

    CliArg {
        name: "cat-file",
        short: '\0',
        on_set: |git| {
            git.set_command(GitCommand::CatFile);
            Ok(())
        }
    },

    CliArg {
        name: "hash-object",
        short: '\0',
        on_set: |git| {
            git.set_command(GitCommand::HashObject);
            Ok(())
        }
    },

    CliArg {
        name: "pretty-print",
        short: 'p',
        on_set: |git| {
            match git.command() {
                GitCommand::CatFile => {
                    git.set_pretty_print(true);
                    Ok(())
                },
                _ => {
                    Err(GitError::CLIError("invalid flag -p".to_string()))
                }
            }
        }
    },

    CliArg {
        name: "write",
        short: 'w',
        on_set: |git| {
            match git.command() {
                GitCommand::HashObject => {
                    git.set_write_object(true);
                    Ok(())
                },
                _ => {
                    Err(GitError::CLIError("invalid flag -p".to_string()))
                }
            }
        }
    },
];

#[derive(Debug, PartialEq)]
enum GitCommand {
    Unset,
    Init,
    CatFile,
    HashObject,
    Help,
}

#[derive(Debug)]
pub struct Git<O> {
    command: GitCommand,
    hash: String,
    pretty_print: bool,
    write_object: bool,
    path: PathBuf,
    out: O, // used to stream commands output directly for performance
}

impl<O> GitInstance for Git<O> {
    fn set_command(&mut self, cmd: GitCommand) {
        self.command = cmd;
    }

    fn set_pretty_print(&mut self, pp: bool) {
        self.pretty_print = pp;
    }

    fn command(&mut self) -> &GitCommand {
        &self.command
    }

    fn set_write_object(&mut self, write_object: bool) {
        self.write_object = write_object;
    }
}

impl Default for Git<Stdout> {
    fn default() -> Self {
        Self {
            command: GitCommand::Unset,
            hash: String::with_capacity(40),
            pretty_print: false,
            out: io::stdout(),
            path: PathBuf::new(),
            write_object: false,
        }
    }
}

impl Git<Stdout> {
    /// Creates a git instance from current env, uses stdout for output
    /// The argument parsing enforces fhis format
    /// $ git COMMAND (FLAGS|POS_ARGS)
    /// short-form flags can be combined as one group, e.g. -pvi
    pub fn from_env() -> Result<Self, GitError> {
        let mut args = env::args();
        let _ = args.next().unwrap(); // binary name
        let mut git = Git::default();

        while let Some(arg) = args.next() {
            if arg.starts_with("-") {
                // ---------------------
                // - short form flag(s)
                if git.command == GitCommand::Unset {
                    return Err(GitError::CLIError("please specify command first then flags".to_string()))
                }

                // process all chars in flag group
                let mut chars = arg.chars();
                let _ = chars.next().unwrap(); // remove flag hyphen
                while let Some(c) = chars.next() {
                    if c == ' ' {
                        break;
                    }
                    if let Some(a) = VALID_CLI_ARGS.iter().find(|a| a.short == c) {
                        (a.on_set)(&mut git)?;
                    } else {
                        return Err(GitError::CLIError(format!("invalid flag -{}", c)))
                    }
                }
            } else if git.command == GitCommand::Unset {
                // ---------------------
                // positional arg where command has yet to be set, validate command
                let _= match VALID_CLI_ARGS.iter().filter(|a| a.name == arg).next() {
                    Some(arg) => (arg.on_set)(&mut git),
                    _ => Err(GitError::CLIError(format!("invalid command: {}", arg))),
                }?;
            } else {
                // -----------------------
                // another positional arg
                git.take_argument(&arg)?;
            }
        }
        if git.command == GitCommand::Unset {
            // if we made it this far without errors and command hasn't been set
            // it means no args were supplied, let's help the user with usage
            git.command = GitCommand::Help;
        }
        Ok(git)
    }

}

impl<O: io::Write> Git<O> {
    pub fn run(&mut self) -> Result<(), GitError> {
        match self.command {
            GitCommand::Init => self.init(),
            GitCommand::CatFile => self.cat_file(),
            GitCommand::HashObject => self.hash_object(),
            GitCommand::Help => todo!(), // implement usage
            GitCommand::Unset => todo!() // implement usage
        }
    }

    /// init commnad
    fn init(&mut self) -> Result<(), GitError> {
        if let Err(e) = Repo::init() {
            return Err(e)
        }
        gprintln!(self, "Initialized git directory")?;
        Ok(())
    }

    /// cat-file commnad
    fn cat_file(&mut self) -> Result<(), GitError> {
        if let Err(e) = Object::cat_object_from_hash(&self.hash, &mut self.out) {
            return Err(e)
        }
        Ok(())
    }

    /// hash-object commnad
    fn hash_object(&mut self) -> Result<(), GitError> {
        if let Err(e) = Object::hash_object_from_file(&self.path, &mut self.out, self.write_object)
        {
            return Err(e);
        }
        Ok(())
    }

    /// parses positional arguments based on established command
    /// this is called during parsing after we have recognized a valid command
    fn take_argument(&mut self, arg: &str) -> Result<(), GitError> {
        match &self.command {
            GitCommand::CatFile if self.hash == "" => self.hash.push_str(&arg),
            GitCommand::HashObject => self.path.push(arg),
            _ => return Err(GitError::CLIError(format!("unexpected positional argument: {}", arg))),
        };
        Ok(())
    }
}

