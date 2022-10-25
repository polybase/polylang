package parser

/*
#cgo darwin LDFLAGS: ${SRCDIR}/libpolylang.a
#cgo linux LDFLAGS: -lpolylang
#include "./polylang.h"
*/
import "C"
import (
	"encoding/json"
	"errors"
	"fmt"
	"strings"
)

type Result[T any] struct {
	Ok  T
	Err *Error
}

type Error struct {
	Message string `json:"message"`
}

func IsAuthError(err error) bool {
	// TODO: refactor this when we make Error more descriptive
	return strings.Contains(err.Error(), "Missing public key from auth")
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

func Interpret(program, contract, funcName, args string) (json.RawMessage, error) {
	output := C.interpret(C.CString(program), C.CString(contract), C.CString(funcName), C.CString(args))
	return parseResult[json.RawMessage](C.GoString(output))
}

func ValidateSet(contractAST, data string) error {
	output := C.validate_set(C.CString(contractAST), C.CString(data))
	if _, err := parseResult[json.RawMessage](C.GoString(output)); err != nil {
		return err
	}

	return nil
}

func ValidateSetDecorators(programAST, contractName, data, previousData, publicKey string) error {
	output := C.validate_set_decorators(C.CString(programAST), C.CString(contractName), C.CString(data), C.CString(previousData), C.CString(publicKey))
	if _, err := parseResult[json.RawMessage](C.GoString(output)); err != nil {
		return err
	}

	return nil
}

func GenerateJSFunction(funcAST string) (EvalInput, error) {
	output := C.generate_js_function(C.CString(funcAST))
	return parseResult[EvalInput](C.GoString(output))
}
