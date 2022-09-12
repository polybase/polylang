package main

import (
	"encoding/json"
	"spacetime/parser"
)

type Program struct {
	Nodes []struct {
		Collection json.RawMessage `json:"Collection"`
	} `json:"nodes"`
}

func main() {
	result := parser.Parse("collection Test { name: string!; }")
	println(result)

	var ast Program
	if err := json.Unmarshal([]byte(result), &ast); err != nil {
		panic(err)
	}

	result = parser.Interpret("collection Test { function get_age(a) { if (a == 41) { return 1; } else { return 2; } } }", "Test", "get_age", `{ "a": { "value": {"Number": 42} } }`)
	println(result)

	result = parser.ValidateSet(string(ast.Nodes[0].Collection), `{ "name": {"Number": 42.0} }`)
	println(result)
}
