#include <stddef.h>

char *parse(const char *input);
char *interpret(const char *program, const char *contract, const char *func, const char *args);
char *validate_set(const char *ast_json, const char *data_json);
char *generate_js_function(const char *func_ast_json);
