/* tslint:disable */
/* eslint-disable */
/**
* @param {string} code
* @param {string | undefined} contract_name
* @param {string} fn_name
* @returns {Program}
*/
export function compile(code: string, contract_name: string | undefined, fn_name: string): Program;
/**
* @param {Uint8Array | undefined} proof
* @param {any} program_info
* @param {any[]} stack_inputs
* @param {any[]} output_stack
* @param {any[]} overflow_addrs
* @returns {boolean}
*/
export function verify(proof: Uint8Array | undefined, program_info: any, stack_inputs: any[], output_stack: any[], overflow_addrs: any[]): boolean;
/**
*/
export function init(): void;
/**
* @param {string} input
* @param {string} namespace
* @returns {string}
*/
export function parse(input: string, namespace: string): string;
/**
* @param {string} ast_json
* @param {string} data_json
* @returns {string}
*/
export function validate_set(ast_json: string, data_json: string): string;
/**
* @param {string} contract_ast_json
* @returns {string}
*/
export function generate_js_contract(contract_ast_json: string): string;
/**
*/
export class Output {
  free(): void;
/**
* @returns {number}
*/
  cycle_count(): number;
/**
* @returns {Uint8Array | undefined}
*/
  proof(): Uint8Array | undefined;
/**
* @returns {any}
*/
  program_info(): any;
/**
* @returns {any[]}
*/
  stack_inputs(): any[];
/**
* @returns {any[]}
*/
  output_stack(): any[];
/**
* @returns {any[]}
*/
  overflow_addrs(): any[];
/**
* @returns {any}
*/
  this(): any;
/**
* @returns {any}
*/
  result(): any;
/**
* @returns {any}
*/
  result_hash(): any;
/**
* @returns {any}
*/
  hashes(): any;
/**
* @returns {any}
*/
  logs(): any;
/**
* @returns {boolean}
*/
  self_destructed(): boolean;
/**
* @returns {boolean}
*/
  read_auth(): boolean;
}
/**
*/
export class Program {
  free(): void;
/**
* @returns {string}
*/
  miden_code(): string;
/**
* @param {string} this_json
* @param {string} args_json
* @param {boolean} generate_proof
* @returns {Output}
*/
  run(this_json: string, args_json: string, generate_proof: boolean): Output;
}
