// generator-typescript-cli — entry point. The export name `generatorApi`
// matches the WIT interface `forge:plugin/generator-api`; jco discovers
// our `info`, `configSchema`, `generate` functions through that namespace.
//
// `result<ok, err>` returns get translated into thrown exceptions on the
// err side. We wrap config-invalid / rejection in `throw.ts`'s helpers so
// the variant tag is correct (a plain `throw new Error(...)` becomes a
// `StageError::PluginBug` trap, which is wrong for typed errors).

import type { GenerationOutput, Ir, PluginInfo, StageError } from './types.js';
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
    // Untrapped JS exceptions become opaque `wasm unreachable` traps in
    // StarlingMonkey, dropping the original message. Catch anything that
    // isn't already a typed `StageError` and re-throw it with the JS
    // error attached so the host gets a useful PluginBug payload.
    try {
      return emitAll(spec, config);
    } catch (e) {
      if (e && typeof e === 'object' && 'tag' in (e as object)) throw e;
      const msg = e instanceof Error ? `${e.name}: ${e.message}\n${e.stack ?? ''}` : String(e);
      // eslint-disable-next-line @typescript-eslint/only-throw-error
      throw { tag: 'plugin-bug', val: msg } satisfies StageError;
    }
  },
};
