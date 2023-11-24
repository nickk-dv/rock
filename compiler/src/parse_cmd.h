#ifndef PARSE_CMD_H
#define PARSE_CMD_H

#include "general/general.h"

i32 parse_cmd(int argc, char** argv);

static bool match_arg(char* arg, const char* match);
static i32 cmd_help();
static i32 cmd_new(char* name);
static i32 cmd_check();
static i32 cmd_build();
static i32 cmd_run();
static i32 cmd_fmt();

#endif
