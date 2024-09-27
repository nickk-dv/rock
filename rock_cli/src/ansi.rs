pub struct AnsiStyle {
    pub out: &'static AnsiColors,
    pub err: &'static AnsiColors,
}

pub struct AnsiColors {
    pub reset: &'static str,
    pub red_bold: &'static str,
    pub green_bold: &'static str,
    pub yellow_bold: &'static str,
    pub cyan: &'static str,
    pub cyan_bold: &'static str,
    pub white_bold: &'static str,
}

const ANSI_COLORS: AnsiColors = AnsiColors {
    reset: "\x1B[0m",
    red_bold: "\x1B[1;31m",
    green_bold: "\x1B[1;32m",
    yellow_bold: "\x1B[1;33m",
    cyan: "\x1B[0;36m",
    cyan_bold: "\x1B[1;36m",
    white_bold: "\x1B[1;37m",
};

const ANSI_COLORS_EMPTY: AnsiColors = AnsiColors {
    reset: "",
    red_bold: "",
    green_bold: "",
    yellow_bold: "",
    cyan: "",
    cyan_bold: "",
    white_bold: "",
};

impl AnsiStyle {
    pub fn new() -> AnsiStyle {
        use std::io::IsTerminal;

        AnsiStyle {
            out: if std::io::stdout().is_terminal() {
                &ANSI_COLORS
            } else {
                &ANSI_COLORS_EMPTY
            },
            err: if std::io::stderr().is_terminal() {
                &ANSI_COLORS
            } else {
                &ANSI_COLORS_EMPTY
            },
        }
    }
}
