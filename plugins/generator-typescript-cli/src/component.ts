// generator-typescript-cli — entry point. The export name `generatorApi`
// matches the WIT interface `forge:plugin/generator-api`; jco discovers
// our `info`, `configSchema`, `generate` functions through that namespace.
//
// `result<ok, err>` returns get translated into thrown exceptions on the
// err side. We wrap config-invalid / rejection in `throw.ts`'s helpers so
// the variant tag is correct (a plain `throw new Error(...)` becomes a
// `StageError::PluginBug` trap, which is wrong for typed errors).

import type { GenerationOutput, Ir, PluginInfo } from './types.js';
import { CONFIG_SCHEMA, parseConfig } from './config.js';
import { emitAll } from './emit/index.js';

export const generatorApi = {
  info(): PluginInfo {
    return {
      name: 'generator-typescript-cli',
      version: '0.1.0',
    };
  },

  configSchema(): string {
    return CONFIG_SCHEMA;
  },

  generate(spec: Ir, configRaw: string): GenerationOutput {
    const config = parseConfig(configRaw);
    return emitAll(spec, config);
  },
};
