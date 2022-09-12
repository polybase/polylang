package parser

/*
#cgo LDFLAGS: ./parser/libspacetime_parser.a
#include "./parser.h"
*/
import "C"

func Parse(input string) string {
	output := C.parse(C.CString(input))
	return C.GoString(output)
}

func Interpret(program, collection, funcName, args string) string {
	output := C.interpret(C.CString(program), C.CString(collection), C.CString(funcName), C.CString(args))
	return C.GoString(output)
}

func ValidateSet(collectionAST, data string) string {
	output := C.validate_set(C.CString(collectionAST), C.CString(data))
	return C.GoString(output)
}
