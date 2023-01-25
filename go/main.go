package main

import (
	"encoding/json"
	"log"

	"github.com/polybase/polylang/parser"
	"github.com/polybase/polylang/stableast"
)

type Program struct {
	Nodes []struct {
		Collection json.RawMessage `json:"Collection"`
	} `json:"nodes"`
}

func main() {
	parseResult, err := parser.Parse("collection Test { name: string; }", "default")
	if err != nil {
		panic(err)
	}

	var ast Program
	if err := json.Unmarshal([]byte(parseResult[0]), &ast); err != nil {
		panic(err)
	}

	var stableAST stableast.Root
	if err := json.Unmarshal([]byte(parseResult[1]), &stableAST); err != nil {
		panic(err)
	}

	err = parser.ValidateSet(string(ast.Nodes[0].Collection), `{ "name": 42.0 }`)
	if err == nil {
		panic("no error from ValidateSet")
	}
	log.Println(err)
}
