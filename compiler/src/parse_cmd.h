#ifndef PARSE_CMD_H
#define PARSE_CMD_H

int parse_cmd(int argc, char** argv);
bool arg_match(char* arg, const char* match);
int cmd_build(char* filepath);

#endif
