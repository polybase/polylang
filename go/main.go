package main

import (
	"encoding/json"
	"log"

	"github.com/spacetimehq/spacetime-parser/parser"
)

type Program struct {
	Nodes []struct {
		Collection json.RawMessage `json:"Collection"`
	} `json:"nodes"`
}

func main() {
	parseResult, err := parser.Parse("collection Test { name: string!; }")
	if err != nil {
		panic(err)
	}
	log.Println(string(parseResult))

	var ast Program
	if err := json.Unmarshal([]byte(parseResult), &ast); err != nil {
		panic(err)
	}

	interpretResult, err := parser.Interpret("collection Test { function get_age(a) { if (a == 41) { return 1; } else { return 2; } } }", "Test", "get_age", `{ "a": { "value": {"Number": 42} } }`)
	if err != nil {
		panic(err)
	}
	log.Println(string(interpretResult))

	err = parser.ValidateSet(string(ast.Nodes[0].Collection), `{ "name": 42.0 }`)
	if err == nil {
		panic("no error from ValidateSet")
	}
	log.Println(err)
}
