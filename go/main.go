package main

import (
	"encoding/json"
	"log"

	"github.com/polybase/polylang/parser"
)

type Program struct {
	Nodes []struct {
		Contract json.RawMessage `json:"Contract"`
	} `json:"nodes"`
}

func main() {
	parseResult, err := parser.Parse("contract Test { name: string!; }")
	if err != nil {
		panic(err)
	}
	log.Println(string(parseResult))

	var ast Program
	if err := json.Unmarshal([]byte(parseResult), &ast); err != nil {
		panic(err)
	}

	interpretResult, err := parser.Interpret("contract Test { function get_age(a: number) { if (a == 41) { return 1; } else { return 2; } } }", "Test", "get_age", `{ "a": { "value": {"Number": 42} } }`)
	if err != nil {
		panic(err)
	}
	log.Println(string(interpretResult))

	err = parser.ValidateSet(string(ast.Nodes[0].Contract), `{ "name": 42.0 }`)
	if err == nil {
		panic("no error from ValidateSet")
	}
	log.Println(err)
}
