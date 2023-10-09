const path = require('path')
const fs = require('fs')
const WasmPackPlugin = require('@wasm-tool/wasm-pack-plugin')

const inlineWASM = (pkgDir) => ({
  apply: (compiler) => {
    compiler.hooks.beforeCompile.tap('InlineWASM', () => {
      const wasm = fs.readFileSync(
        path.resolve(__dirname, pkgDir + '/index_bg.wasm'),
      )

      const js = `
        import * as index_bg from "./index_bg.js"

        const base64 = "${Buffer.from(wasm).toString('base64')}"

        function toUint8Array (s) {
          return new Uint8Array(atob(s).split('').map(c => c.charCodeAt(0)))
        }

        const wasm = toUint8Array(base64)

        const { instance } = await WebAssembly.instantiate(wasm, {
          "./index_bg.js": index_bg,
        })

        export default instance.exports
      `

      fs.writeFileSync(path.resolve(__dirname, pkgDir + '/index_bg.wasm.js'), js)

      const index = fs.readFileSync(
        path.resolve(__dirname, pkgDir + '/index_bg.js'),
      )

      fs.writeFileSync(
        path.resolve(__dirname, pkgDir + '/index_bg.js'),
        index
          .toString()
          .replace(
            'let wasm;',
            'import wasm from \'./index_bg.js\'',
          ),
      )

      fs.unlinkSync(path.resolve(__dirname, pkgDir + '/index_bg.wasm'))
    })
  },
})

module.exports = {
  // ...

  plugins: [
    new WasmPackPlugin({
      crateDirectory: path.resolve(__dirname, '.'),

      // Check https://rustwasm.github.io/wasm-pack/book/commands/build.html for
      // the available set of arguments.
      //
      // Optional space delimited arguments to appear before the wasm-pack
      // command. Default arguments are `--verbose`.
      args: '--log-level warn',
      // Default arguments are `--typescript --target browser --mode normal`.

      // Optional array of absolute paths to directories, changes to which
      // will trigger the build.
      // watchDirectories: [
      //   path.resolve(__dirname, "another-crate/src")
      // ],

      // The same as the `--out-dir` option for `wasm-pack`
      // outDir: "pkg",

      // The same as the `--out-name` option for `wasm-pack`
      // outName: "index",

      // If defined, `forceWatch` will force activate/deactivate watch mode for
      // `.rs` files.
      //
      // The default (not set) aligns watch mode for `.rs` files to Webpack's
      // watch mode.
      // forceWatch: true,

      // If defined, `forceMode` will force the compilation mode for `wasm-pack`
      //
      // Possible values are `development` and `production`.
      //
      // the mode `development` makes `wasm-pack` build in `debug` mode.
      // the mode `production` makes `wasm-pack` build in `release` mode.
      // forceMode: "development",

      // Controls plugin output verbosity, either 'info' or 'error'.
      // Defaults to 'info'.
      // pluginLogLevel: 'info'
    }),
    inlineWASM('pkg'),
  ],

  // ...
}