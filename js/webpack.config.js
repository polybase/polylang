const path = require('path')
const fs = require('fs')
const nodeExternals = require('webpack-node-externals')
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
            'import * as wasm from \'./index_bg.wasm\'',
            'import wasm from \'./index_bg.wasm.js\'',
          ),
      )

      fs.unlinkSync(path.resolve(__dirname, pkgDir + '/index_bg.wasm'))
    })
  },
})

module.exports = {
  target: 'web',
  entry: {
    index: './src/index.ts',
    'validator/index': './src/validator/index.ts',
  },
  experiments: {
    topLevelAwait: true,
  },
  resolve: {
    extensions: ['.ts', '.js'],
  },
  externals: [nodeExternals(), { buffer: 'buffer' }],
  module: {
    rules: [
      {
        test: /\.ts$/,
        loader: 'ts-loader',
        options: {
          compilerOptions: {
            outDir: path.resolve(__dirname, 'dist'),
          },
        },
      },
    ],
  },
  plugins: [
    new WasmPackPlugin({
      crateDirectory: path.resolve(__dirname, '..'),
      outDir: path.resolve(__dirname, 'pkg'),
    }),
    inlineWASM('pkg'),
    new WasmPackPlugin({
      crateDirectory: path.resolve(__dirname, '..'),
      outDir: path.resolve(__dirname, 'pkg-thin'),
      extraArgs: '-- --no-default-features --features bindings',
    }),
    inlineWASM('pkg-thin'),
  ],
  output: {
    path: path.resolve(__dirname, 'dist'),
    filename: '[name].js',
    libraryTarget: 'commonjs2',
  },
}
