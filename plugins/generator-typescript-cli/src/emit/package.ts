// Generated package.json + tsconfig.json + bin shim. Output uses commander
// 14 for subcommand routing and only Node ≥20 built-ins otherwise (`fetch`,
// `node:http`, `node:crypto`, `URLSearchParams`); no runtime deps beyond
// commander itself.

import { TSCONFIG_JSON } from './static_files.js';

export interface PackageInputs {
  packageName: string;     // e.g. "github-issues-cli"
  binName: string;         // e.g. "github-issues"
  version: string;         // from spec info.version (SemVer-coerced)
  description: string;     // from spec info.description / title
}

export function packageJson(inp: PackageInputs): string {
  const obj = {
    name: inp.packageName,
    version: inp.version,
    description: inp.description,
    type: 'module',
    bin: { [inp.binName]: `bin/${inp.binName}.js` },
    main: 'src/index.ts',
    scripts: {
      build: 'tsc',
      typecheck: 'tsc --noEmit',
    },
    dependencies: {
      commander: '^14.0.0',
    },
    devDependencies: {
      typescript: '^5.5.0',
      '@types/node': '^22.0.0',
    },
    engines: { node: '>=20' },
  };
  return JSON.stringify(obj, null, 2) + '\n';
}

export function tsconfig(): string {
  return TSCONFIG_JSON;
}

// Shebang shim that tsx-style runs the compiled CLI. The shim invokes the
// already-compiled dist/cli.js (after `npm run build`); for an unbuilt
// install we fall back to importing src/cli.ts directly via Node's
// experimental loader. Keeping it simple: compile-then-run is the
// primary path.
export function binShim(): string {
  return `#!/usr/bin/env node
// Generated CLI entry. Run \`npm run build\` to compile src/ → dist/.
import('../dist/cli.js').catch((err) => {
  if (err && err.code === 'ERR_MODULE_NOT_FOUND') {
    console.error('CLI not built. Run: npm install && npm run build');
    process.exit(2);
  }
  console.error(err);
  process.exit(1);
});
`;
}
