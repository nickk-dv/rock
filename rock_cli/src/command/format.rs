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

    let format: CommandFormat = CommandFormat {
        name: parse_name(&mut p, &mut diagnostics),
        args: parse_args(&mut p),
        options: parse_options(&mut p, &mut diagnostics),
        trail_args: p.trail_args(),
    };
    ResultComp::new(format, diagnostics)
}

fn parse_name(p: &mut FormatParser, diagnostics: &mut DiagnosticCollection) -> String {
    if let Some(name) = p.eat_arg() {
        name
    } else {
        diagnostics.error(ErrorComp::message(
            "command name is missing, use `rock help` to learn the usage",
        ));
        "error".into()
    }
}

fn parse_args(p: &mut FormatParser) -> Vec<String> {
    let mut cmd_args = Vec::new();

    while let Some(arg) = p.eat_arg() {
        cmd_args.push(arg);
    }
    cmd_args
}

fn parse_options(
    p: &mut FormatParser,
    diagnostics: &mut DiagnosticCollection,
) -> HashMap<String, Vec<String>> {
    let mut cmd_options = HashMap::new();
    let mut duplicates = HashSet::new();

    while let Some(opt_name) = p.eat_option() {
        let mut opt_args = Vec::new();
        while let Some(arg) = p.eat_arg() {
            opt_args.push(arg);
        }

        if cmd_options.contains_key(&opt_name) {
            duplicates.insert(opt_name);
        } else {
            cmd_options.insert(opt_name, opt_args);
        }
    }

    for duplicate in duplicates {
        diagnostics.warning(WarningComp::message(format!(
            "duplicate `--{duplicate}` options will be ignored"
        )));
    }
    cmd_options
}

struct FormatParser {
    cursor: usize,
    args: Vec<String>,
}

impl FormatParser {
    fn new() -> FormatParser {
        FormatParser {
            cursor: 0,
            args: std::env::args().skip(1).collect(),
        }
    }

    fn eat_arg(&mut self) -> Option<String> {
        let arg = self.args.get(self.cursor)?;

        if arg.starts_with("--") {
            None
        } else {
            self.cursor += 1;
            Some(arg.clone())
        }
    }

    fn eat_option(&mut self) -> Option<String> {
        let arg = self.args.get(self.cursor)?;
        let option = arg.strip_prefix("--")?;

        self.cursor += 1;
        if option.is_empty() {
            None
        } else {
            Some(option.to_string())
        }
    }

    fn trail_args(mut self) -> Vec<String> {
        self.args.split_off(self.cursor)
    }
}
