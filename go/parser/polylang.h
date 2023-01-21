#include <stddef.h>

char *parse(const char *input, const char *namespace);
char *validate_set(const char *ast_json, const char *data_json);
char *generate_js_collection(const char *collection_ast_json);
