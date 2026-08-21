import { readFile } from 'node:fs/promises';
import { WASI } from 'node:wasi';
import { argv, env } from 'node:process';
const wasi = new WASI({
  version: 'preview1',
  args: argv.slice(2),
  env,
  preopens: { '/': '/' },
});
const wasm = await WebAssembly.compile(await readFile(argv[2]));
const instance = await WebAssembly.instantiate(wasm, wasi.getImportObject());
process.exit(wasi.start(instance));
