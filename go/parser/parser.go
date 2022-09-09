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

func Interpret(program string, collection string, funcName string, args string) string {
	output := C.interpret(C.CString(program), C.CString(collection), C.CString(funcName), C.CString(args))
	return C.GoString(output)
}
