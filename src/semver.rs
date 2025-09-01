
use std::{fmt::Debug, io};
use std::fmt;
use jsonpath_rust::JsonPath;
use luaparse::ast::Expr;
use luaparse::ast::Statement::If;
use luaparse::token::TokenValue::Symbol;
use std::error::Error;
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
    has_v_prefix: bool,
}

#[derive(Debug)]
struct QueryTraversalResult {
    identifiers: Vec<String>,
    comparators: Vec<luaparse::token::Symbol>,
    literals: Vec<String>,
    connectors: Vec<luaparse::token::Symbol>,
    errors: Vec<String>,
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        //TODO: add prerelease and build number, if they are supported
        let mut sem_ver: String; 
        if self.has_v_prefix {
            sem_ver = format!("v{}.{}.{}", self.major, self.minor, self.patch);
        } else {
            sem_ver = format!("{}.{}.{}", self.major, self.minor, self.patch);
        }   
        match self.pre_release.clone() {
            Some(pre_release) => {
                sem_ver.push_str(format!("-{}", pre_release).as_str());
            }
            _ => {}
        }

        match self.build_number.clone() {
            Some(build_number) => {
                sem_ver.push_str(format!("+{}", build_number).as_str());
            }
            _ => {}
        }

        write!(f, "{}", sem_ver)
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

pub fn query_semver(query: &String, semver_entries: Vec<String>, strict: bool) -> Result<Vec<String>, Box<dyn Error>> {
    // source: https://github.com/semver/semver/blob/master/semver.md?plain=1#L346
    let semver_regex = Regex::new(r"^(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)(?:-(?P<prerelease>(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+(?P<buildmetadata>[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$")?;

    let mut parsed_sem_vers :Vec<SemVer> = vec![];

    let mut has_v_prefix: bool = false;
    for mut semver in semver_entries {
        if semver.starts_with("v") {
            semver = semver.replacen("v", "", 1);
            has_v_prefix = true;
        }
        if !semver_regex.is_match(semver.as_str()) {
            if strict {
                return Err(Box::new(io::Error::new(io::ErrorKind::InvalidInput, format!("{semver} does not follow the semantic versioning format"))));
            }
            continue
        }

        let captures = semver_regex.captures(semver.as_str()).
        ok_or(Box::new(io::Error::new(io::ErrorKind::InvalidInput, "failed parsing input")))?;

        parsed_sem_vers.push(SemVer{
            major: *(&captures["major"].parse::<u16>()?),
            minor: *(&captures["minor"].parse::<u16>()?),
            patch: *(&captures["patch"].parse::<u16>()?),
            pre_release: captures.name("prerelease").map(|m|m.as_str().to_string()),
            build_number: captures.name("buildmetadata").map(|m|m.as_str().to_string()),
            has_v_prefix: has_v_prefix,
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

    match parse(buf.as_str()) {
        Ok(block) => {
            //println!("statement length: {}", block.statements.len());

            match block.statements[5].clone() {
                If(if_statement) => {
                    //println!("the condition: {:?}", if_statement.condition);
                    let mut traversal_result = QueryTraversalResult::new();
                    traverse_bin_op_expression(if_statement.condition, &mut traversal_result);
                    if traversal_result.errors.len() > 0 {
                        return Err(Box::new(io::Error::new(io::ErrorKind::InvalidInput, format!("error parsing query: {:?}", traversal_result.errors))));
                    }
                    // TODO: add an additional check here on variable names: only major, minor, patch are supported for now
                    let jsonpath_query = convert_to_jsonpath_syntax(&traversal_result);
                    // Parse the string of data into serde_json::Value.
                    let json = serde_json::to_string(&parsed_sem_vers)?;
                    let v: Value = serde_json::from_str(json.as_str())?;  
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
            return Err(Box::new(io::Error::new(io::ErrorKind::InvalidInput, "unable to parse query")));
        }
    }

    Ok(vec![String::new()])
}


fn traverse_bin_op_expression(node: Expr, traversal_result: &mut QueryTraversalResult)  {
    match node {
        Expr::BinOp(binop_expr) => {
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
        Expr::String(string) => {
            traversal_result.literals.push(string.to_string())
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
        jsonpath_query.push_str(comparator_to_jsonpath_string(traversal_result.comparators[i]));
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

fn comparator_to_jsonpath_string(smbl: luaparse::token::Symbol) -> &'static str {
    if smbl == luaparse::token::Symbol::NotEqual {
        return "!="
    }

    return smbl.as_str()
}



#[cfg(test)]
mod tests {

    use std::{fs};

    use super::*;

    struct FailureTestCase {
        query: &'static str,
        static_input_data: Vec<&'static str>,
        error_message: Option<&'static str>,
    }


    #[test]
    fn invalid_query_returns_error() {
        let input_data = "0.1.1";
        let cases: Vec<FailureTestCase> = vec![FailureTestCase{
            query: "$%^",
            static_input_data: vec![input_data],
            error_message: Some("unable to parse query"),

        }, FailureTestCase{
            query: "a + b",
            static_input_data: vec![input_data],
            // TODO: fix error message
            error_message: Some("error parsing query: [\"unsupported boolean operator Symbol(Add), only >, >=, <, <=, ==, ~= are supported\"]"),

        }, FailureTestCase{
            query: "minor += major",
            static_input_data: vec![input_data],
            error_message: Some("unable to parse query"),
        }, FailureTestCase{
            query: "minor === major",
            static_input_data: vec![input_data],
            error_message: Some("unable to parse query"),
        }
        ];
        for i in 0..cases.len() {
            let input: Vec<String> = cases[i].static_input_data.iter().map(|x|String::from(*x)).collect();
            match query_semver(&cases[i].query.to_string(), input, true) {
                Ok(_) => {
                    assert!(false, "case {} failed: error is expected for query: {}", i, cases[i].query)
                },
                Err(err) => {
                    assert_eq!(err.to_string(), cases[i].error_message.unwrap(), "case {} failed", i)
                }
            }
        }
    }

    #[test]
    fn invalid_semver_format_returns_error() {
        let query = "major >= 1";
        let cases: Vec<FailureTestCase> = vec![FailureTestCase{
            query: query,
            static_input_data: vec!["foo bar"],
            error_message: Some("foo bar does not follow the semantic versioning format"),

        }, FailureTestCase{
            query: query,
            static_input_data: vec!["x.1.0"],
            error_message: Some("x.1.0 does not follow the semantic versioning format"),

        }, FailureTestCase{
            query: query,
            static_input_data: vec!["1.x.2"],
            error_message: Some("1.x.2 does not follow the semantic versioning format"),
        }, FailureTestCase{
            query: query,
            static_input_data: vec!["1.2.x"],
            error_message: Some("1.2.x does not follow the semantic versioning format"),
        }
        ];
        for i in 0..cases.len() {
            let input: Vec<String> = cases[i].static_input_data.iter().map(|x|String::from(*x)).collect();
            match query_semver(&cases[i].query.to_string(), input, true) {
                Ok(_) => {
                    assert!(false, "case {} failed: error is expected for query: {}", i, cases[i].query)
                },
                Err(err) => {
                    assert_eq!(err.to_string(), cases[i].error_message.unwrap(), "case {} failed", i)
                }
            }
        }
    }

    #[test]
    fn queries_set_1() -> Result<(), Box<dyn Error>>{
      let input_data  = fs::read_to_string("src/test_data/keycloak/input.txt")?;
      let input_set: Vec<String> = input_data.lines().map(|ln|String::from(ln)).collect();

        let queries: Vec<&str> = vec![
            "major >= 20",
            "major <= 20 and minor > 0",
            "major == 26 and minor > 0 and patch > 0",
            "major >= 23 and major <= 26 and minor > 0",
            "major >= 17 and major <= 20 and patch > 1 and patch <= 3",
        ];
        for i in 0..queries.len() {
            let expectation_file = fs::read_to_string(format!("src/test_data/keycloak/case_{i}_expectation.txt"))?;
            let expected_result: Vec<String> = expectation_file.lines().map(|ln|String::from(ln)).collect();

            match query_semver(&String::from(queries[i]),
            input_set.clone(), true) {
                Ok(actual_result) => {
                    assert_eq!(actual_result, expected_result, "case {} failed: expected: {:?}, got: {:?}", i, expected_result, actual_result);
                },
                Err(err) => {
                    assert!(false, "case {} failed: error occurred: {}", i, err);
                }
            }
        }
        Ok(())
    }

     #[test]
    fn queries_set_2() -> Result<(), Box<dyn Error>> {
      let input_data  = fs::read_to_string("src/test_data/kubernetes/input.txt")?;
      let input_set: Vec<String> = input_data.lines().map(|ln|String::from(ln)).collect();

        let queries: Vec<&str> = vec![
            "minor >= 30",
            "minor >= 29 and patch > 0",
            "minor == 34",
            "minor == 29 and pre_release ~= 'alpha.0' and pre_release ~= 'alpha.1' and pre_release ~= 'alpha.2' and pre_release ~= 'alpha.3'",
            "patch == 7",
        ];
        for i in 0..queries.len() {
            let expectation_file = fs::read_to_string(format!("src/test_data/kubernetes/case_{i}_expectation.txt"))?;
            let expected_result: Vec<String> = expectation_file.lines().map(|ln|String::from(ln)).collect();

            match query_semver(&String::from(queries[i]),
            input_set.clone(), true) {
                Ok(actual_result) => {
                    assert_eq!(actual_result, expected_result, "case {} failed: expected: {:?}, got: {:?}", i, expected_result, actual_result);
                },
                Err(err) => {
                    assert!(false, "case {} failed: error occurred: {}", i, err);
                }
            }
        }
        Ok(())
    }

    #[test]
    fn queries_set_3() -> Result<(), Box<dyn Error>>{
      let input_data  = fs::read_to_string("src/test_data/tensorflow/input.txt")?;
      let input_set: Vec<String> = input_data.lines().map(|ln|String::from(ln)).collect();

        let queries: Vec<&str> = vec![
            "major == 2 and minor == 0",
            "major == 1 and minor == 15 and patch > 0",
            "major == 2 and minor >= 7 and minor <= 9",
            "minor == 17 and patch > 0",
        ];
        for i in 0..queries.len() {
            let expectation_file = fs::read_to_string(format!("src/test_data/tensorflow/case_{i}_expectation.txt"))?;
            let expected_result: Vec<String> = expectation_file.lines().map(|ln|String::from(ln)).collect();

            match query_semver(&String::from(queries[i]),
            input_set.clone(), true) {
                Ok(actual_result) => {
                    assert_eq!(actual_result, expected_result, "case {} failed: expected: {:?}, got: {:?}", i, expected_result, actual_result);
                },
                Err(err) => {
                    assert!(false, "case {} failed: error occurred: {}", i, err);
                }
            }
        }
        Ok(())
    }
}