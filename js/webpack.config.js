const path = require('path')
const nodeExternals = require('webpack-node-externals')
const WasmPackPlugin = require('@wasm-tool/wasm-pack-plugin')

const config = (target) => ({
  target,
  entry: './src/index.ts',
  experiments: {
    asyncWebAssembly: true,
  },
  resolve: {
    extensions: ['.ts', '.js'],
  },
  externals: [nodeExternals()],
  module: {
    rules: [
      {
        test: /\.ts$/,
        loader: 'ts-loader',
        options: {
          compilerOptions: {
            outDir: path.resolve(__dirname, target),
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
  ],
  output: {
    path: path.resolve(__dirname, target),
    filename: 'index.js',
    libraryTarget: 'commonjs2',
  },
})

module.exports = [config('node'), config('web')]
