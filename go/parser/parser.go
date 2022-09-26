package parser

/*
#cgo darwin LDFLAGS: ${SRCDIR}/libspacetime_lang.a
#cgo linux LDFLAGS: -lspacetime_lang
#include "./spacetime_lang.h"
*/
import "C"
import (
	"encoding/json"
	"errors"
	"fmt"
)

type Result[T any] struct {
	Ok  T
	Err *Error
}

type Error struct {
	Message string `json:"message"`
}

type EvalInput struct {
	Code string `json:"code"`
}

func parseResult[T any](resultJSON string) (T, error) {
	var result Result[T]
	if err := json.Unmarshal([]byte(resultJSON), &result); err != nil {
		return result.Ok, fmt.Errorf("failed to parse result: %w", err)
	}

	if result.Err != nil {
		return result.Ok, errors.New(result.Err.Message)
	}

	return result.Ok, nil
}

func Parse(input string) (json.RawMessage, error) {
	output := C.parse(C.CString(input))
	return parseResult[json.RawMessage](C.GoString(output))
}

func Interpret(program, collection, funcName, args string) (json.RawMessage, error) {
	output := C.interpret(C.CString(program), C.CString(collection), C.CString(funcName), C.CString(args))
	return parseResult[json.RawMessage](C.GoString(output))
}

func ValidateSet(collectionAST, data string) error {
	output := C.validate_set(C.CString(collectionAST), C.CString(data))
	if _, err := parseResult[json.RawMessage](C.GoString(output)); err != nil {
		return err
	}

	return nil
}

func GenerateJSFunction(funcAST string) (EvalInput, error) {
	output := C.generate_js_function(C.CString(funcAST))
	return parseResult[EvalInput](C.GoString(output))
}
