
use std::fmt::Debug;
use std::io;
use std::fs;
use std::fmt;
use std::io::Write;
use std::str::FromStr;
use std::process::ExitCode;
use jsonpath_rust::JsonPath;
use luaparse::ast::Expr;
use luaparse::ast::Statement::If;
use luaparse::token::TokenValue::Symbol;
use std::error::Error;
use clap::{Command, Arg, value_parser};
use regex::Regex;
use luaparse::{parse};
use serde::{Deserialize, Serialize};
use serde_json::{Value};


#[derive(Debug,Serialize, Deserialize)]
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
    comparators: Vec<luaparse::token::Symbol>,
    literals: Vec<String>,
    connectors: Vec<luaparse::token::Symbol>,
    errors: Vec<String>,
}

#[derive(Debug, Clone)]
struct SemVerParseError{
    msg: String,
}

impl fmt::Display for SemVerParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        //TODO: add prerelease and build number if they are supported
        write!(f, "{}.{}.{}", self.major, self.minor, self.minor)
    }
}

impl QueryTraversalResult {
    pub fn new() -> QueryTraversalResult {
        QueryTraversalResult {
            identifiers: Vec::new(),
            comparators: Vec::new(),
            literals: Vec::new(),
            connectors: Vec::new(), 
            errors: Vec::new(),
        }
    }
}

impl SemVerParseError {
    pub fn new(msg: String) -> SemVerParseError {
        SemVerParseError {
            msg,
        }
    }
}

impl Error for SemVerParseError {}


fn main() -> ExitCode {
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


       // println!("{:?}", matches.get_one::<String>("query").unwrap());

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


   match query_semver(matches.get_one::<String>("query").unwrap(), input.unwrap().lines().collect()) {
    Ok(query) => {
        write!(io::stdout(), "{:?}", query);
        ExitCode::SUCCESS
    },
    Err(err) => {
        write!(io::stderr(), "{}", err);
        ExitCode::FAILURE
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


/*     for parsed_sem_ver in parsed_sem_vers {
        println!("{:#?}", parsed_sem_ver);
    } */

    match parse(buf.as_str()) {
        Ok(block) => {
            println!("statement length: {}", block.statements.len());

            match block.statements[5].clone() {
                If(if_statement) => {
                    //println!("the condition: {:?}", if_statement.condition);
                    let mut traversal_result = QueryTraversalResult::new();
                    traverse_bin_op_expression(if_statement.condition, &mut traversal_result);
                    println!("{:#?}", traversal_result);
                    if traversal_result.errors.len() > 0 {
                        return Err(Box::new(SemVerParseError::new(format!("error parsing query: {:?}", traversal_result.errors))));
                    }
                    // TODO: add an additional check here on variable names: only major, minor, patch are supported for now
                    let jsonpath_query = convert_to_jsonpath_syntax(&traversal_result);
                    println!("{}", jsonpath_query);
                    // Parse the string of data into serde_json::Value.
                    let v: Value = serde_json::from_str(serde_json::to_string(&parsed_sem_vers)?.as_str())?;  
                    let res_json: Vec<Value> =  v.query(jsonpath_query.as_str())?.iter().map(|v| (*v).clone()).collect();
                    let mut final_result: Vec<String> = Vec::new();
                    for val in res_json {
                        final_result.push(serde_json::from_value::<SemVer>(val)?.to_string());
                    }

                    return Ok(final_result);
                }
                _ => {}
            }
        }
        Err(err) => {
            // print error only for debugging
             // println!("{}", err);
            return Err(Box::new(SemVerParseError::new(String::from("unable to parse query"))));
        }
    }

    Ok(vec![String::new()])
}


fn traverse_bin_op_expression(node: Expr, traversal_result: &mut QueryTraversalResult)  {
    match node {
        Expr::BinOp(binop_expr) => {
            //println!("4. left inside: {}", binop_expr.left.to_string());
            //println!("5. op inside: {:?}", binop_expr.op.0.token.value);
            //println!("6. right inside: {:?}", binop_expr.right);
            //println!("==============================================");

              if let Expr::BinOp(_) = *binop_expr.left {
                match binop_expr.op.0.token.value.clone() {
                    Symbol(luaparse::token::Symbol::And) => {traversal_result.connectors.push(luaparse::token::Symbol::And)},
                    Symbol(luaparse::token::Symbol::Or) => {traversal_result.connectors.push(luaparse::token::Symbol::Or)},
                    x => {traversal_result.errors.push(format!("unsupported logical operator {:?}, only and/or are supported", x))}
                }
                
                match *binop_expr.right {
                    Expr::BinOp(_) => {}
                    _ => {
                      traversal_result.errors.push(format!("invalid expression {}", binop_expr.right.to_string()));
                    }
                }
              } else {
                match binop_expr.op.0.token.value.clone() {
                    Symbol(luaparse::token::Symbol::Greater) => {traversal_result.comparators.push(luaparse::token::Symbol::Greater)},
                    Symbol(luaparse::token::Symbol::GreaterEqual) => {traversal_result.comparators.push(luaparse::token::Symbol::GreaterEqual)},
                    Symbol(luaparse::token::Symbol::Less) => {traversal_result.comparators.push(luaparse::token::Symbol::Less)},
                    Symbol(luaparse::token::Symbol::LessEqual) => {traversal_result.comparators.push(luaparse::token::Symbol::LessEqual)},
                    Symbol(luaparse::token::Symbol::Equal) => {traversal_result.comparators.push(luaparse::token::Symbol::Equal)},
                    Symbol(luaparse::token::Symbol::NotEqual) => {traversal_result.comparators.push(luaparse::token::Symbol::NotEqual)},
                    x => {traversal_result.errors.push(format!("unsupported boolean operator {:?}, only >, >=, <, <=, ==, ~= are supported", x))}
                }
              }

            traverse_bin_op_expression(*binop_expr.clone().left, traversal_result);
            traverse_bin_op_expression(*binop_expr.clone().right, traversal_result);
        }
        Expr::Prefix(prefix_exp) => {
            traversal_result.identifiers.push(prefix_exp.to_string())
        }
        Expr::Number(number_lit) => {
            traversal_result.literals.push(number_lit.to_string())
        }
        _ => {

        }
    }
}


fn convert_to_jsonpath_syntax(traversal_result: &QueryTraversalResult) -> String {
    let mut jsonpath_query = String::from("$[?");

    for i in 0..traversal_result.identifiers.len() {
        jsonpath_query.push_str("@.");
        jsonpath_query.push_str(traversal_result.identifiers[i].as_str());
        jsonpath_query.push_str(" ");
        jsonpath_query.push_str(traversal_result.comparators[i].as_str());
        jsonpath_query.push_str(" ");
        jsonpath_query.push_str(traversal_result.literals[i].as_str());
        if i != traversal_result.identifiers.len() -1 {
            jsonpath_query.push_str(" ");
            jsonpath_query.push_str(lua_boolean_operator_to_jsonpath_string(traversal_result.connectors[i]));
            jsonpath_query.push_str(" ");
        }
    }

    jsonpath_query.push_str("]");
    jsonpath_query
}

fn lua_boolean_operator_to_jsonpath_string(symbol: luaparse::token::Symbol) -> &'static str {
    match symbol {
        luaparse::token::Symbol::And => {"&&"}
        luaparse::token::Symbol::Or => {"||"}
        _ => {"unknown"}
    }
} 