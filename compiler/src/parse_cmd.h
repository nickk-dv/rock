#ifndef PARSE_CMD_H
#define PARSE_CMD_H

#include "common.h"

i32 parse_cmd(int argc, char** argv);

static bool match_arg(char* arg, const char* match);
static i32 cmd_build(char* filepath);

#endif
