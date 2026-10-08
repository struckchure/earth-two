import { build } from 'esbuild';
import { mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const output = fileURLToPath(new URL('../../build/spacetime/', import.meta.url));
await mkdir(output, { recursive: true });
await build({ entryPoints: ['bridge.ts'], bundle: true, format: 'iife', target: 'es2022', outfile: output + 'account.js', minify: true });
