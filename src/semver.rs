
use std::{fmt::Debug, io};
use std::fmt;
use luaparse::ast::{BinOpExpr, Expr, Name, NumberLit, PrefixExpr, StringLit, Var};
use luaparse::ast::Statement::If;
use luaparse::token::{NumberLiteral, Symbol as symb, TokenValue};
use luaparse::token::TokenValue::Symbol;
use std::error::Error;
use regex::Regex;
use luaparse::{parse};
use std::cmp::Ordering;
use std::str::FromStr;


pub const ASCENDING_ORDER: &str = "asc";
pub const DESCENDING_ORDER: &str = "desc";
pub const NO_ORDER: &str = "none";


#[derive(Debug)]
struct SemVer {
    major: u16,
    minor: u16,
    patch: u16,
    pre_release: Option<String>, 
    build_number: Option<String>,
    has_v_prefix: bool,
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        let core = self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch));

        /////// done by copilot
        if core != Ordering::Equal {
            return core;
        }

         match (&self.pre_release, &other.pre_release) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater, // final > prerelease
            (Some(_), None) => Ordering::Less,    // prerelease < final
            (Some(a), Some(b)) => compare_prerelease(a, b),
        }
        ///////////
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for SemVer {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major && self.minor == other.minor &&
        self.patch == other.patch && 
        self.pre_release == other.pre_release
    }
}

impl Eq for SemVer {}

#[derive(Copy, Clone)]
pub enum SortOrder {
    Ascending,Descending,None
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseSortOrderError;

impl FromStr for SortOrder {
    type Err = ParseSortOrderError;
    fn from_str(order: &str) -> Result<Self, Self::Err> {
        match order {
            ASCENDING_ORDER => {
                Ok(SortOrder::Ascending)
            }
            DESCENDING_ORDER => {
                Ok(SortOrder::Descending)
            }
            NO_ORDER => {
               Ok(SortOrder::None) 
            }
            _ => {
                Err(ParseSortOrderError)
            }
        }
    }
}

impl fmt::Display for SortOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let order: &str;
        match self {
            SortOrder::Ascending => {
                order = ASCENDING_ORDER;
            }
            SortOrder::Descending => {
                order = DESCENDING_ORDER;
            }
            SortOrder::None => {
              order = NO_ORDER;
            }
        }

        write!(f, "{}", order)
    }
}

/// A parsed query condition, evaluated directly against each `SemVer`.
#[derive(Debug)]
enum Condition {
    Compare { field: Field, op: symb, literal: Literal },
    And(Box<Condition>, Box<Condition>),
    Or(Box<Condition>, Box<Condition>),
}

#[derive(Debug, Clone, Copy)]
enum Field {
    Major,
    Minor,
    Patch,
    PreRelease,
    BuildNumber,
}

#[derive(Debug)]
enum Literal {
    Number(f64),
    String(Vec<u8>),
}

enum FieldValue<'a> {
    Number(f64),
    Text(&'a str),
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

// function written by copilot
fn is_numeric_identifier(identifier: &str) -> bool {
    !identifier.is_empty() && identifier.chars().all(|c| c.is_ascii_digit())
}

// function written by copilot
fn compare_prerelease(a: &str, b: &str) -> Ordering {
    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();
    for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
        let a_is_num = is_numeric_identifier(a_part);
        let b_is_num = is_numeric_identifier(b_part);
        match (a_is_num, b_is_num) {
            (true, true) => {
                // Both numeric: compare by numeric value
                let a_num = a_part.parse::<u64>().unwrap_or(0);
                let b_num = b_part.parse::<u64>().unwrap_or(0);
                let ord = a_num.cmp(&b_num);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (true, false) => {
                // Numeric identifiers have lower precedence than non-numeric
                return Ordering::Less;
            }
            (false, true) => {
                return Ordering::Greater;
            }
            (false, false) => {
                let ord = a_part.cmp(b_part);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
    // All shared identifiers equal; shorter prerelease has lower precedence
    a_parts.len().cmp(&b_parts.len())
}


pub fn query_semver(query: &String, semver_entries: Vec<String>, strict: bool, sort_order: SortOrder, limit: usize) -> Result<Vec<String>, Box<dyn Error>> {
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

    match sort_order {
         SortOrder::Ascending => {
            parsed_sem_vers.sort();
        }
         SortOrder::Descending => {
            parsed_sem_vers.sort_by(|a, b| b.cmp(a));
        }
        _ => {}
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
                    let mut errors: Vec<String> = Vec::new();
                    let condition = match parse_condition(&if_statement.condition, &mut errors) {
                        Some(condition) if errors.is_empty() => condition,
                        _ => return Err(Box::new(io::Error::new(io::ErrorKind::InvalidInput, format!("error parsing query: {:?}", errors)))),
                    };
                    let final_result: Vec<String>; 
                    let filtered = parsed_sem_vers
                        .iter()
                        .filter(|sem_ver| evaluate(&condition, sem_ver));

                    if limit != 0 {
                        final_result = filtered.take(limit)
                        .map(|sem_ver| sem_ver.to_string())
                        .collect();
                        //final_result.truncate(limit);
                    } else {
                        final_result = filtered
                        .map(|sem_ver| sem_ver.to_string())
                        .collect();
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


/// Builds a `Condition` from the Lua AST of the query, collecting errors into `errors`.
fn parse_condition(node: &Expr, errors: &mut Vec<String>) -> Option<Condition> {
    if let Expr::Prefix(PrefixExpr::Parenthesized(parenthesized)) = node {
        return parse_condition(&parenthesized.expr, errors);
    }

    let Expr::BinOp(binop) = node else {
        errors.push(format!("invalid expression {}", node.to_string()));
        return None;
    };

    let symbol = match &binop.op.0.token.value {
        Symbol(symbol) => *symbol,
        _ => {
            errors.push(String::from("invalid syntax"));
            return None;
        }
    };

    match symbol {
        symb::And | symb::Or => {
            let left = parse_condition(&binop.left, errors);
            let right = parse_condition(&binop.right, errors);
            let (left, right) = (left?, right?);
            Some(if symbol == symb::And {
                Condition::And(Box::new(left), Box::new(right))
            } else {
                Condition::Or(Box::new(left), Box::new(right))
            })
        }
        symb::Greater | symb::GreaterEqual | symb::Less | symb::LessEqual | symb::Equal | symb::NotEqual => {
            parse_comparison(node, binop, symbol, errors)
        }
        other => {
            errors.push(format!("unsupported operator {}, only >, >=, <, <=, ==, ~= are supported", symb::to_string(&other)));
            None
        }
    }
}

/// Builds a `Condition::Compare` from `<identifier> <op> <literal>`.
fn parse_comparison(node: &Expr, binop: &BinOpExpr, op: symb, errors: &mut Vec<String>) -> Option<Condition> {
    let Some(name) = identifier_name(&binop.left) else {
        errors.push(format!("invalid expression {}", node.to_string()));
        return None;
    };
    let Some(field) = Field::from_name(name) else {
        errors.push(format!("unsupported identifier `{}`, only major, minor, patch, pre_release and build_number are supported", name));
        return None;
    };
    let Some(literal) = literal_value(&binop.right) else {
        errors.push(format!("invalid expression {}", node.to_string()));
        return None;
    };
    Some(Condition::Compare { field, op, literal })
}

fn identifier_name<'a>(node: &Expr<'a>) -> Option<&'a str> {
    match node {
        Expr::Prefix(PrefixExpr::Var(Var::Name(Name(reference)))) => match reference.token.value {
            TokenValue::Ident(name) => Some(name),
            _ => None,
        },
        _ => None,
    }
}

fn literal_value(node: &Expr) -> Option<Literal> {
    match node {
        Expr::Number(NumberLit(reference)) => match &reference.token.value {
            TokenValue::Number(NumberLiteral::Integer(value)) => Some(Literal::Number(*value as f64)),
            TokenValue::Number(NumberLiteral::Float(value)) => Some(Literal::Number(*value)),
            _ => None,
        },
        Expr::String(StringLit(reference)) => match &reference.token.value {
            TokenValue::String { value, .. } => Some(Literal::String(value.to_vec())),
            _ => None,
        },
        _ => None,
    }
}

fn evaluate(condition: &Condition, sem_ver: &SemVer) -> bool {
    match condition {
        Condition::And(left, right) => evaluate(left, sem_ver) && evaluate(right, sem_ver),
        Condition::Or(left, right) => evaluate(left, sem_ver) || evaluate(right, sem_ver),
        Condition::Compare { field, op, literal } => {
            let ordering = match (field.value_of(sem_ver), literal) {
                (Some(FieldValue::Number(value)), Literal::Number(expected)) => value.partial_cmp(expected),
                (Some(FieldValue::Text(value)), Literal::String(expected)) => Some(value.as_bytes().cmp(expected.as_slice())),
                // a missing field or a type mismatch is never equal and never ordered
                _ => None,
            };
            match ordering {
                Some(ordering) => match *op {
                    symb::Greater => ordering == Ordering::Greater,
                    symb::GreaterEqual => ordering != Ordering::Less,
                    symb::Less => ordering == Ordering::Less,
                    symb::LessEqual => ordering != Ordering::Greater,
                    symb::Equal => ordering == Ordering::Equal,
                    symb::NotEqual => ordering != Ordering::Equal,
                    _ => false,
                },
                None => *op == symb::NotEqual,
            }
        }
    }
}

impl Field {
    fn from_name(name: &str) -> Option<Field> {
        match name {
            "major" => Some(Field::Major),
            "minor" => Some(Field::Minor),
            "patch" => Some(Field::Patch),
            "pre_release" => Some(Field::PreRelease),
            "build_number" => Some(Field::BuildNumber),
            _ => None,
        }
    }

    fn value_of<'a>(self, sem_ver: &'a SemVer) -> Option<FieldValue<'a>> {
        match self {
            Field::Major => Some(FieldValue::Number(sem_ver.major.into())),
            Field::Minor => Some(FieldValue::Number(sem_ver.minor.into())),
            Field::Patch => Some(FieldValue::Number(sem_ver.patch.into())),
            Field::PreRelease => sem_ver.pre_release.as_deref().map(FieldValue::Text),
            Field::BuildNumber => sem_ver.build_number.as_deref().map(FieldValue::Text),
        }
    }
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
            error_message: Some("error parsing query: [\"unsupported operator `+`, only >, >=, <, <=, ==, ~= are supported\"]"),
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
            match query_semver(&cases[i].query.to_string(), input, true, SortOrder::None, 0) {
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
            match query_semver(&cases[i].query.to_string(), input, true, SortOrder::None, 0) {
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
            for order in [SortOrder::None, SortOrder::Ascending, SortOrder::Descending] {
                let expectation_file = fs::read_to_string(format!("src/test_data/keycloak/case_{i}_expectation_{order}.txt"))?;
                let expected_result: Vec<String> = expectation_file.lines().map(|ln|String::from(ln)).collect();

                match query_semver(&String::from(queries[i]),
                input_set.clone(), true, order, 0) {
                    Ok(actual_result) => {
                        assert_eq!(actual_result, expected_result, "case {} failed (sort: {}): expected: {:?}, got: {:?}", i, order, expected_result, actual_result);
                        //assert!(actual_result.is_sorted())
                    },
                    Err(err) => {
                        assert!(false, "case {} failed: error occurred: {}", i, err);
                    }
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
            for order in [SortOrder::None, SortOrder::Ascending, SortOrder::Descending] {
                let expectation_file = fs::read_to_string(format!("src/test_data/kubernetes/case_{i}_expectation_{order}.txt"))?;
                let expected_result: Vec<String> = expectation_file.lines().map(|ln|String::from(ln)).collect();

                match query_semver(&String::from(queries[i]),
                input_set.clone(), true, order, 0) {
                    Ok(actual_result) => {
                        assert_eq!(actual_result, expected_result, "case {} failed (sort: {}): expected: {:?}, got: {:?}", i, order, 
                        expected_result, actual_result);
                    },
                    Err(err) => {
                        assert!(false, "case {} failed: error occurred: {}", i, err);
                    }
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
            for order in [SortOrder::None, SortOrder::Ascending, SortOrder::Descending] {
                let expectation_file = fs::read_to_string(format!("src/test_data/tensorflow/case_{i}_expectation_{order}.txt"))?;
                let expected_result: Vec<String> = expectation_file.lines().map(|ln|String::from(ln)).collect();

                match query_semver(&String::from(queries[i]),
                input_set.clone(), true, order, 0) {
                    Ok(actual_result) => {
                        assert_eq!(actual_result, expected_result, "case {} failed (sort: {}): expected: {:?}, got: {:?}", i, order, expected_result, actual_result);
                    },
                    Err(err) => {
                        assert!(false, "case {} failed: error occurred: {}", i, err);
                    }
                }
            }
        }
        Ok(())
    }

       #[test]
    fn with_limit() -> Result<(), Box<dyn Error>>{
      let input_data  = fs::read_to_string("src/test_data/kubernetes/input.txt")?;
      let input_set: Vec<String> = input_data.lines().map(|ln|String::from(ln)).collect();

        let limits = vec![10, 5, 13, 7, 16];
        for i in 0..limits.len() {
            let expected_limit = limits[i];
                match query_semver(&String::from("major == 1"),
                input_set.clone(), true, SortOrder::None, expected_limit) {
                    Ok(actual_result) => {
                        assert_eq!(actual_result.len(), limits[i], "case {} failed: expected result size: {}, got: {}", i, limits[i], actual_result.len());
                    },
                    Err(err) => {
                        assert!(false, "case {} failed: error occurred: {}", i, err);
                    }
            }
        }
        Ok(())
    }

    #[test]
    fn parenthesized_groups_are_respected() -> Result<(), Box<dyn Error>> {
        let input: Vec<String> = vec!["1.0.0", "2.0.0", "2.1.0", "3.0.0"].into_iter().map(String::from).collect();
        let result = query_semver(&String::from("(major == 1) or (major == 2 and minor == 1)"), input, true, SortOrder::None, 0)?;
        assert_eq!(result, vec!["1.0.0", "2.1.0"]);
        Ok(())
    }

    #[test]
    fn unknown_identifier_returns_error() {
        let input: Vec<String> = vec![String::from("1.0.0")];
        match query_semver(&String::from("foo > 1"), input, true, SortOrder::None, 0) {
            Ok(_) => assert!(false, "error is expected for an unknown identifier"),
            Err(err) => assert_eq!(err.to_string(), "error parsing query: [\"unsupported identifier `foo`, only major, minor, patch, pre_release and build_number are supported\"]"),
        }
    }
}
