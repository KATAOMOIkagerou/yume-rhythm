import { buildEnvironment, run } from './toolchain.mjs';
const { env, cargo } = buildEnvironment();
const [command, ...args] = process.argv.slice(2);
run(cargo, [command, '--manifest-path', 'native/Cargo.toml', ...args], env);
