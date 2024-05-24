use rock_core::error::{DiagnosticCollection, ErrorComp, ResultComp, WarningComp};
use std::collections::{HashMap, HashSet};

pub struct CommandFormat {
    pub name: String,
    pub args: Vec<String>,
    pub options: HashMap<String, Vec<String>>,
    pub trail_args: Vec<String>,
}

pub fn parse() -> ResultComp<CommandFormat> {
    let mut p = FormatParser::new();
    let mut diagnostics = DiagnosticCollection::new();

    let cmd_name = if let Some(name) = p.eat_arg() {
        name
    } else {
        diagnostics.error(ErrorComp::message(
            "command name is missing, use `rock help` to learn the usage",
        ));
        "error".into()
    };

    let mut cmd_args = Vec::new();
    while let Some(arg) = p.eat_arg() {
        cmd_args.push(arg);
    }

    let mut duplicates = HashSet::new();
    let mut cmd_options = HashMap::new();

    while let Some(name) = p.eat_option() {
        let opt_name = name;

        let mut opt_args = Vec::new();
        while let Some(arg) = p.eat_arg() {
            opt_args.push(arg);
        }

        if cmd_options.get(&opt_name).is_some() {
            duplicates.insert(opt_name);
        } else {
            cmd_options.insert(opt_name, opt_args);
        }
    }

    // order of elements in hashmap gets random
    for duplicate in duplicates {
        diagnostics.warning(WarningComp::message(format!(
            "duplicate options `--{duplicate}` will be ignored"
        )));
    }

    let format = CommandFormat {
        name: cmd_name,
        args: cmd_args,
        options: cmd_options,
        trail_args: p.trail_args(),
    };
    ResultComp::new(format, diagnostics)
}

struct FormatParser {
    args: Vec<String>,
    cursor: usize,
}

impl FormatParser {
    fn new() -> FormatParser {
        FormatParser {
            args: std::env::args().skip(1).collect(),
            cursor: 0,
        }
    }

    fn eat_arg(&mut self) -> Option<String> {
        match self.args.get(self.cursor) {
            Some(arg) => {
                if arg.starts_with("--") {
                    None
                } else {
                    self.cursor += 1;
                    Some(arg.clone())
                }
            }
            None => None,
        }
    }

    fn eat_option(&mut self) -> Option<String> {
        match self.args.get(self.cursor) {
            Some(arg) => {
                if let Some(option) = arg.strip_prefix("--") {
                    self.cursor += 1;
                    if option.is_empty() {
                        None
                    } else {
                        Some(option.to_string())
                    }
                } else {
                    None
                }
            }
            None => None,
        }
    }

    fn trail_args(mut self) -> Vec<String> {
        self.args.split_off(self.cursor)
    }
}
