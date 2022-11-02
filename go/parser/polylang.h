#include <stddef.h>

char *parse(const char *input);
char *interpret(const char *program, const char *collection, const char *func, const char *args);
char *validate_set(const char *ast_json, const char *data_json);
char *generate_js_collection(const char *collection_ast_json);
