use rock_core::error::ErrorComp;
use std::collections::{HashMap, HashSet};

pub struct CommandFormat {
    pub name: String,
    pub args: Vec<String>,
    pub options: HashMap<String, Vec<String>>,
    pub trail_args: Vec<String>,
}

pub fn parse() -> Result<CommandFormat, ErrorComp> {
    let mut p = FormatParser::new();
    // maybe report duplicates as warnings when proper warning support is done @25.04.24
    // currently duplicate options are ignored
    let mut duplicates = HashSet::new();

    let cmd_name = if let Some(name) = p.peek_arg() {
        name
    } else {
        return Err(ErrorComp::message(
            "command name is missing, use `rock help` to learn the usage",
        ));
    };

    let mut cmd_args = Vec::new();
    while let Some(arg) = p.peek_arg() {
        cmd_args.push(arg);
    }

    let mut cmd_options = HashMap::new();
    while let Some(name) = p.peek_option() {
        let opt_name = name;

        let mut opt_args = Vec::new();
        while let Some(arg) = p.peek_arg() {
            opt_args.push(arg);
        }

        if cmd_options.get(&opt_name).is_some() {
            duplicates.insert(opt_name);
        } else {
            cmd_options.insert(opt_name, opt_args);
        }
    }

    Ok(CommandFormat {
        name: cmd_name,
        args: cmd_args,
        options: cmd_options,
        trail_args: p.trail_args(),
    })
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

    fn peek_arg(&mut self) -> Option<String> {
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

    fn peek_option(&mut self) -> Option<String> {
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
