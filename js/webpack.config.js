const path = require('path')
const fs = require('fs')
const nodeExternals = require('webpack-node-externals')
const WasmPackPlugin = require('@wasm-tool/wasm-pack-plugin')

module.exports = {
  target: 'web',
  entry: './src/index.ts',
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
    {
      apply: (compiler) => {
        compiler.hooks.beforeCompile.tap('InlineWASM', () => {
          const wasm = fs.readFileSync(
            path.resolve(__dirname, 'pkg/index_bg.wasm'),
          )

          const js = `
            import * as index_bg from "./index_bg.js"

            const base64 = "${Buffer.from(wasm).toString('base64')}"

            function toUint8Array (s) {
              if (typeof atob === 'function') return new Uint8Array(atob(s).split('').map(c => c.charCodeAt(0)))
              return (require('buffer').Buffer).from(s, 'base64')
            }

            const wasm = toUint8Array(base64)

            const { instance } = await WebAssembly.instantiate(wasm, {
              "./index_bg.js": index_bg,
            })

            export default instance.exports
          `

          fs.writeFileSync(path.resolve(__dirname, 'pkg/index_bg.wasm.js'), js)

          const index = fs.readFileSync(
            path.resolve(__dirname, 'pkg/index_bg.js'),
          )

          fs.writeFileSync(
            path.resolve(__dirname, 'pkg/index_bg.js'),
            index
              .toString()
              .replace(
                'import * as wasm from \'./index_bg.wasm\'',
                'import wasm from \'./index_bg.wasm.js\'',
              ),
          )

          fs.unlinkSync(path.resolve(__dirname, 'pkg/index_bg.wasm'))
        })
      },
    },
  ],
  output: {
    path: path.resolve(__dirname, 'dist'),
    filename: 'index.js',
    libraryTarget: 'commonjs2',
  },
}
