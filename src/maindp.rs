#![allow(dead_code)]
extern crate crossbeam_channel as channel;

use std::borrow::ToOwned;
use std::env;
use std::fmt;
use std::io;
use std::process;
use std::time::Instant;

use docopt::Docopt;
use log::{error, info, log_enabled, Level};
use serde::Deserialize;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

macro_rules! wout {
    ($($arg:tt)*) => ({
        use std::io::Write;
        (writeln!(&mut ::std::io::stdout(), $($arg)*)).unwrap();
    });
}

macro_rules! werr {
    ($($arg:tt)*) => ({
        use std::io::Write;
        (writeln!(&mut ::std::io::stderr(), $($arg)*)).unwrap();
    });
}

macro_rules! fail {
    ($e:expr) => {
        Err(::std::convert::From::from($e))
    };
}

macro_rules! command_list {
    () => {
        "
    count       Count records
    dedup       Remove redundant rows
    excel       Exports an Excel sheet to a CSV
    exclude     Excludes the records in one CSV from another
    frequency   Show frequency tables
    help        Show this usage message
    index       Create CSV index for faster access
    input       Read CSV data with special quoting rules
    pseudo      Pseudonymise the values of a column
    replace     Replace patterns in CSV data
    sample      Randomly sample CSV data
    search      Search CSV data with a regex
    searchset   Search CSV data with a regex set
    select      Select, re-order, duplicate or drop columns
    slice       Slice records from CSV
    sort        Sort CSV data in alphabetical, numerical, reverse or random order
    stats       Infer data types and compute descriptive statistics

    sponsored by datHere - Data Infrastructure Engineering
"
    };
}
mod cmd;
mod config;
mod index;
mod select;
mod util;

static USAGE: &str = concat!(
    "
Usage:
    qsvdp <command> [<args>...]
    qsvdp [options]

Options:
    --list               List all commands available.
    --envlist            List all qsv-relevant environment variables.
    -h, --help           Display this message
    <command> -h         Display the command help message
    -v, --version        Print version info, mem allocator, features installed, 
                         max_jobs, num_cpus then exit

* sponsored by datHere - Data Infrastructure Engineering
"
);

#[derive(Deserialize)]
struct Args {
    arg_command: Option<Command>,
    flag_list: bool,
    flag_envlist: bool,
}

fn main() {
    util::init_logger();

    let now = Instant::now();
    let mut qsv_args: String = env::args().skip(1).collect::<Vec<_>>().join(" ");
    if log_enabled!(Level::Info) {
        info!("START: {qsv_args}");
    }

    let args: Args = Docopt::new(USAGE)
        .and_then(|d| {
            d.options_first(true)
                .version(Some(util::version()))
                .deserialize()
        })
        .unwrap_or_else(|e| e.exit());
    if args.flag_list {
        wout!(concat!("Installed commands:", command_list!()));
        return;
    } else if args.flag_envlist {
        util::show_env_vars();
        return;
    }
    match args.arg_command {
        None => {
            werr!(concat!(
                "qsvdp is a suite of CSV command line utilities optimized for Datapusher+.

Please choose one of the following commands:",
                command_list!()
            ));
            ::std::process::exit(0);
        }
        Some(cmd) => match cmd.run() {
            Ok(()) => {
                if log_enabled!(Level::Info) {
                    let ellipsis = if qsv_args.len() > 15 {
                        qsv_args.truncate(15);
                        "..."
                    } else {
                        ""
                    };
                    info!(
                        "END \"{qsv_args}{ellipsis}\" elapsed: {}",
                        now.elapsed().as_secs_f32()
                    );
                }
                process::exit(0);
            }
            Err(CliError::Flag(err)) => err.exit(),
            Err(CliError::Csv(err)) => {
                if log_enabled!(Level::Error) {
                    error!("{err}");
                } else {
                    werr!("{err}");
                }
                process::exit(1);
            }
            Err(CliError::Io(ref err)) if err.kind() == io::ErrorKind::BrokenPipe => {
                process::exit(0);
            }
            Err(CliError::Io(err)) => {
                if log_enabled!(Level::Error) {
                    error!("{err}");
                } else {
                    werr!("{err}");
                }
                process::exit(1);
            }
            Err(CliError::Other(msg)) => {
                if log_enabled!(Level::Error) {
                    error!("{msg}");
                } else {
                    werr!("{msg}");
                }
                process::exit(1);
            }
        },
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Command {
    Count,
    Dedup,
    Excel,
    Exclude,
    Frequency,
    Help,
    Index,
    Input,
    Pseudo,
    Replace,
    Sample,
    Search,
    SearchSet,
    Select,
    Slice,
    Sort,
    Stats,
}

impl Command {
    fn run(self) -> CliResult<()> {
        let argv: Vec<_> = env::args().collect();
        let argv: Vec<_> = argv.iter().map(|s| &**s).collect();
        let argv = &*argv;

        if !argv[1].chars().all(char::is_lowercase) {
            return Err(CliError::Other(format!(
                "qsvdp expects commands in lowercase. Did you mean '{}'?",
                argv[1].to_lowercase()
            )));
        }
        match self {
            Command::Count => cmd::count::run(argv),
            Command::Dedup => cmd::dedup::run(argv),
            Command::Excel => cmd::excel::run(argv),
            Command::Exclude => cmd::exclude::run(argv),
            Command::Frequency => cmd::frequency::run(argv),
            Command::Help => {
                wout!("{USAGE}");
                Ok(())
            }
            Command::Index => cmd::index::run(argv),
            Command::Input => cmd::input::run(argv),
            Command::Pseudo => cmd::pseudo::run(argv),
            Command::Replace => cmd::replace::run(argv),
            Command::Sample => cmd::sample::run(argv),
            Command::Search => cmd::search::run(argv),
            Command::SearchSet => cmd::searchset::run(argv),
            Command::Select => cmd::select::run(argv),
            Command::Slice => cmd::slice::run(argv),
            Command::Sort => cmd::sort::run(argv),
            Command::Stats => cmd::stats::run(argv),
        }
    }
}

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub enum CliError {
    Flag(docopt::Error),
    Csv(csv::Error),
    Io(io::Error),
    Other(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CliError::Flag(ref e) => e.fmt(f),
            CliError::Csv(ref e) => e.fmt(f),
            CliError::Io(ref e) => e.fmt(f),
            CliError::Other(ref s) => f.write_str(&**s),
        }
    }
}

impl From<docopt::Error> for CliError {
    fn from(err: docopt::Error) -> CliError {
        CliError::Flag(err)
    }
}

impl From<csv::Error> for CliError {
    fn from(err: csv::Error) -> CliError {
        if !err.is_io_error() {
            return CliError::Csv(err);
        }
        match err.into_kind() {
            csv::ErrorKind::Io(v) => From::from(v),
            _ => unreachable!(),
        }
    }
}

impl From<io::Error> for CliError {
    fn from(err: io::Error) -> CliError {
        CliError::Io(err)
    }
}

impl From<String> for CliError {
    fn from(err: String) -> CliError {
        CliError::Other(err)
    }
}

impl<'a> From<&'a str> for CliError {
    fn from(err: &'a str) -> CliError {
        CliError::Other(err.to_owned())
    }
}

impl From<regex::Error> for CliError {
    fn from(err: regex::Error) -> CliError {
        CliError::Other(format!("{err:?}"))
    }
}