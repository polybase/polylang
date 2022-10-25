#include <stddef.h>

char *parse(const char *input);
char *interpret(const char *program, const char *contract, const char *func, const char *args);
char *validate_set(const char *ast_json, const char *data_json);
char *validate_set_decorators(const char *program_ast_json, const char *contract_name, const char *data_json, const char *previous_data_json, const char *public_key);
char *generate_js_function(const char *func_ast_json);
