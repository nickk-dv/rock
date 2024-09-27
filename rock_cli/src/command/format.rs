use std::collections::{HashMap, HashSet};

pub struct CommandFormat {
    pub name: Option<String>,
    pub args: Vec<Arg>,
    pub options: HashMap<Opt, Vec<Arg>>,
    pub duplicates: HashSet<Opt>,
    pub trail_args: Vec<String>,
}

pub type Arg = String;
pub type Opt = String;

struct FormatParser {
    args: Vec<String>,
}

pub fn parse() -> CommandFormat {
    let mut p = FormatParser {
        args: std::env::args().skip(1).rev().collect(),
    };

    let name = eat_arg(&mut p);
    let args = parse_args(&mut p);
    let (options, duplicates) = parse_options(&mut p);
    let trail_args = p.args;

    CommandFormat {
        name,
        args,
        options,
        duplicates,
        trail_args,
    }
}

fn parse_args(p: &mut FormatParser) -> Vec<Arg> {
    let mut args = Vec::new();

    while let Some(arg) = eat_arg(p) {
        args.push(arg);
    }
    args
}

fn parse_options(p: &mut FormatParser) -> (HashMap<Opt, Vec<Arg>>, HashSet<Opt>) {
    let mut options = HashMap::new();
    let mut duplicates = HashSet::new();

    while let Some(opt) = eat_option(p) {
        let args = parse_args(p);
        if options.contains_key(&opt) {
            duplicates.insert(opt);
        } else {
            options.insert(opt, args);
        }
    }
    (options, duplicates)
}

fn eat_arg(p: &mut FormatParser) -> Option<Arg> {
    let next = p.args.last()?;
    if next.starts_with('-') {
        None
    } else {
        p.args.pop()
    }
}

fn eat_option(p: &mut FormatParser) -> Option<Opt> {
    let next = p.args.last()?;
    if next == "--" {
        p.args.pop();
        None
    } else {
        let opt = next.trim_start_matches('-').to_string();
        p.args.pop();
        Some(opt)
    }
}
