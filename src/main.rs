use std::io;
use std::fs;
use std::io::IsTerminal;
use std::io::Write;
use std::process::ExitCode;
use clap::{Command, Arg, ArgAction, value_parser};

pub mod semver;


fn main() -> ExitCode {
    let matches = Command::new("semver-query")
        .about("utility for querying data that follows the semantic version format. It expects a list of line separated entries.")
        .version("0.0.1")
        .arg(
            Arg::new("query").
            short('q').
            long("query").
            help("defines the query to apply to the input").
            value_parser(value_parser!(String)).
            required(true).num_args(1)
        ).arg(
            Arg::new("strict").
            long("strict").
            help("defines whether to fail if an entry does not match the semantic versioning regular pattern").
            default_value("true").
            action(ArgAction::SetTrue).
            required(false)
        ).arg(
            Arg::new("filename").help("the input file name. It must contain line separated entries. \nIf not provided, the program attempts to read from the standard input.").required(false)
        ).get_matches();

        let  input: Vec<String> = match matches.get_one::<String>("filename") {
            Some(filename) => {
               match fs::read_to_string(filename) {
                Ok(data) => {
                    data.lines().map(|ln|String::from(ln)).collect()}
                Err(err) => {
                    writeln!(io::stderr(), "{}", err).unwrap();
                    return ExitCode::FAILURE;
                }
               }
            },
            None => {
                let mut lines: Vec<String> = Vec::new();
                if io::stdin().is_terminal() {
                       writeln!(io::stderr(), "no filename provided, input from stdin is empty.").unwrap();
                       return ExitCode::FAILURE;
                } else {
                    for line in io::stdin().lines() {
                        lines.push(line.unwrap());
                    }
                    lines
                }
            }
        };


    match semver::query_semver(matches.get_one::<String>("query").unwrap(), input, *matches.get_one::<bool>("strict").unwrap()) {
    Ok(query_result) => {
        for res_item in query_result {
            println!("{}", res_item);
        } 
        ExitCode::SUCCESS
    },
    Err(err) => {
        writeln!(io::stderr(), "{}", err).unwrap();
        ExitCode::FAILURE
    }
   }
}

