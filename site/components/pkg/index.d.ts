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
* @returns {boolean}
*/
  verify(): boolean;
/**
* @returns {any}
*/
  this(): any;
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
* @param {string} this_json
* @param {string} args_json
* @param {boolean} generate_proof
* @returns {Output}
*/
  run(this_json: string, args_json: string, generate_proof: boolean): Output;
}
