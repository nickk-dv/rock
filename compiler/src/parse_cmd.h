#ifndef PARSE_CMD_H
#define PARSE_CMD_H

int parse_cmd(int argc, char** argv);

static bool match_arg(char* arg, const char* match);
static int cmd_build(char* filepath);

#endif
