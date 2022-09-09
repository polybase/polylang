package main

import (
	"spacetime/parser"
)

func main() {
	result := parser.Parse("collection Test { name: string!; }")
	println(result)

	result = parser.Interpret("collection Test { function get_age(a) { if (a == 41) { return 1; } else { return 2; } } }", "Test", "get_age", `{ "a": { "value": {"Number": 42} } }`)
	println(result)
}
