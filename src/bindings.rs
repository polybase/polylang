use std::os::raw::c_char;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(msg: String);
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn init() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn parse(input: &str) -> String {
    crate::parse_out_json(input)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn interpret(program: &str, collection_name: &str, func: &str, args: &str) -> String {
    let args = serde_json::from_str(args).unwrap();
    crate::interpret_out_json(program, collection_name, func, args)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn validate_set(ast_json: &str, data_json: &str) -> String {
    crate::validate_set_out_json(ast_json, data_json)
}

#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn parse(input: *const c_char) -> *mut c_char {
    let input = unsafe { std::ffi::CStr::from_ptr(input) };
    let input = input.to_str().unwrap();

    let output = crate::parse_out_json(input);
    let output = std::ffi::CString::new(output).unwrap();
    output.into_raw()
}

#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn interpret(
    program: *const c_char,
    collection_name: *const c_char,
    func: *const c_char,
    args: *const c_char,
) -> *mut c_char {
    let program = unsafe { std::ffi::CStr::from_ptr(program) };
    let program = program.to_str().unwrap();

    let collection_name = unsafe { std::ffi::CStr::from_ptr(collection_name) };
    let collection_name = collection_name.to_str().unwrap();

    let func = unsafe { std::ffi::CStr::from_ptr(func) };
    let func = func.to_str().unwrap();

    let args = unsafe { std::ffi::CStr::from_ptr(args) };
    let args = serde_json::from_str(args.to_str().unwrap()).unwrap();

    let output = crate::interpret_out_json(program, collection_name, func, args);
    let output = std::ffi::CString::new(output).unwrap();
    output.into_raw()
}

#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn validate_set(ast_json: *const c_char, data_json: *const c_char) -> *mut c_char {
    let ast_json = unsafe { std::ffi::CStr::from_ptr(ast_json) };
    let ast_json = ast_json.to_str().unwrap();

    let data_json = unsafe { std::ffi::CStr::from_ptr(data_json) };
    let data_json = data_json.to_str().unwrap();

    let output = crate::validate_set_out_json(ast_json, data_json);
    let output = std::ffi::CString::new(output).unwrap();
    output.into_raw()
}
