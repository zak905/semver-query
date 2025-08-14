
use std::any::Any;
use std::fmt::Debug;
use std::io;
use std::fs;
use std::fmt;
use std::str::FromStr;
use luaparse::ast::BinOp;
use luaparse::ast::Expr;
use luaparse::ast::{IfStat};
use luaparse::ast::Statement::If;
use std::error::Error;
use clap::{Command, Arg, value_parser};
use regex::Regex;
use luaparse::{parse};


#[derive(Debug)]
struct SemVer {
    major: u16,
    minor: u16,
    patch: u16,
    pre_release: Option<String>, 
    build_number: Option<String>,
}

#[derive(Debug)]
struct QueryTraversalResult {
    identifiers: Vec<String>,
    comparators: Vec<String>,
    literals: Vec<String>,
    connectors: Vec<String>,
}

#[derive(Debug, Clone)]
struct SemVerParseError{
    item: String,
}

impl fmt::Display for SemVerParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "unable to parse {}", self.item)
    }
}

impl SemVerParseError {
    pub fn new(item_with_err: String) -> SemVerParseError {
        SemVerParseError {
            item: item_with_err,
        }
    }
}

impl SemVerParseError {
    pub fn new(item_with_err: String) -> SemVerParseError {
        SemVerParseError {
            item: item_with_err,
        }
    }
}

impl Error for SemVerParseError {}


fn main() -> Result<(), Box<dyn Error>> {
    let matches = Command::new("semver-query")
        .about("utility for querying a line separated list of entries.")
        .version("0.0.1")
        .arg(
            Arg::new("query").
            short('q').
            long("query").
            help("defines the query to apply to the input").
            value_parser(value_parser!(String)).
            required(true).num_args(1)
        ).arg(
            Arg::new("filename").help("the input file name. It must contain line separated entries.")
        ).get_matches();


        println!("{:?}", matches.get_one::<String>("query").unwrap());

        let  input: Result<String, io::Error> = match matches.get_one::<String>("filename") {
            Some(filename) => {
               match fs::read_to_string(filename) {
                Ok(data) => {Ok(data)}
                Err(err) => {Err(err)}
               }
            },
            None => {
                io::read_to_string(io::stdin())
            }
        };


   match query_semver(matches.get_one::<String>("query").unwrap(), input?.lines().collect()) {
    Ok(query) => {
        print!("{:?}", query);
        Ok(())
    },
    Err(err) => {
        println!("here: {}", err);
        Err(err)
    }
   }
}

fn query_semver(query: &String, semver_entries: Vec<&str>) -> Result<Vec<String>, Box<dyn Error>> {
    print!("{:?}", semver_entries);
    // source: https://github.com/semver/semver/blob/master/semver.md?plain=1#L346
    let semver_regex = Regex::new(r"^(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)(?:-(?P<prerelease>(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+(?P<buildmetadata>[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$")?;

    let mut parsed_sem_vers :Vec<SemVer> = vec![];

    for semver in semver_entries {
        if !semver_regex.is_match(semver) {
            return Err(Box::new(SemVerParseError::new(String::from_str(semver)?)));
        }

        let captures = semver_regex.captures(semver).
        ok_or(Box::new(SemVerParseError::new(String::from_str(semver)?)))?;

        parsed_sem_vers.push(SemVer{
            major: *(&captures["major"].parse::<u16>()?),
            minor: *(&captures["minor"].parse::<u16>()?),
            patch: *(&captures["patch"].parse::<u16>()?),
            pre_release: captures.name("prerelease").map(|m|m.as_str().to_string()),
            build_number: captures.name("buildmetadata").map(|m|m.as_str().to_string()),
        });
    }


let buf = format!(r#"
local major
local minor
local patch
local prerelease
local buildmetadata 

if {query} then
    print("query")
end
"#);


    for parsed_sem_ver in parsed_sem_vers {
        println!("{:#?}", parsed_sem_ver);
    }

    match parse(buf.as_str()) {
        Ok(block) => {
            println!("statement length: {}", block.statements.len());

            match block.statements[5].clone() {
                If(if_statement) => {
                    //println!("the condition: {:?}", if_statement.condition);
                    traverse_bin_op_expression(if_statement.condition);
/*                     if let Expr::BinOp(binopexpr)= if_statement.condition {
                        let mut right = *binopexpr.clone().right;
                        let mut left: Expr<'_> = *binopexpr.clone().left;
                        println!("left: {}", binopexpr.left.to_string());
                        println!("op: {:?}", binopexpr.op.0.token.value);
                        traverse_bin_op_expression(*binopexpr.left);
                        traverse_bin_op_expression(*binopexpr.right);
                     loop {
                            if let Expr::BinOp(ref current) = left {
                                println!("1. left inside: {}", current.left.to_string());
                                println!("2. op inside: {:?}", current.op.0.token.value);
                                println!("3. right inside: {}", current.right.to_string());
                                 left = *current.clone().left;
                            } else {
                                break
                            }
                        } 
                       
                       loop {
                            if let Expr::BinOp(ref current) = right {
                                println!("4. left inside: {}", current.left.to_string());
                                println!("5. op inside: {:?}", current.op.0.token.value);
                                println!("6. right inside: {}", current.right.to_string());
                                 right = *current.clone().right;
                            } else {
                                break
                            }
                        }  
                    } */
                }
                _ => {}
            }
        }
        Err(err) => {
            println!("here here");
            println!("{:?}", err)
        }
    }

    Ok(vec![String::new()])
}


fn traverse_bin_op_expression(node: Expr)  {
     
    if let Expr::BinOp(ref current) = node {
        let type_id = std::any::type_name_of_val(&*current.left);
                                println!("4. left inside: {}, {}", current.left.to_string(), type_id);
                                println!("5. op inside: {:?}", current.op.0.token.value);
                                println!("6. right inside: {}", current.right.to_string());
                                 println!("==============================================");
                                 traverse_bin_op_expression(*current.clone().left);
                                 traverse_bin_op_expression(*current.clone().right);
                                 //current.right.to_string()
                                 //left = *current.clone().left;
                            } else {
                                println!("second case: {}", node.to_string());
                            }

}
