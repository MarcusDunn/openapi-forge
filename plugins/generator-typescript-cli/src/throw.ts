// jco translates `result<ok, err>` returns into thrown exceptions on the
// err path. The thrown value must be the variant value the host expects —
// `{ tag: 'config-invalid', val: '...' }`, not a JS Error.
//
// Always go through these helpers; constructing the variant inline is
// error-prone (typo'd tags become `PluginBug` traps instead of typed
// errors).

import type { StageError, Diagnostic } from './types.js';

export function throwConfigInvalid(message: string): never {
  // eslint-disable-next-line @typescript-eslint/only-throw-error
  throw { tag: 'config-invalid', val: message } satisfies StageError;
}

export function throwRejected(reason: string, diagnostics: Diagnostic[] = []): never {
  // eslint-disable-next-line @typescript-eslint/only-throw-error
  throw { tag: 'rejected', val: { reason, diagnostics } } satisfies StageError;
}

export function throwPluginBug(message: string): never {
  // eslint-disable-next-line @typescript-eslint/only-throw-error
  throw { tag: 'plugin-bug', val: message } satisfies StageError;
}
