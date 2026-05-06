import { throwConfigInvalid } from './throw.js';

export interface OAuthConfig {
  /** OAuth client ID for the public CLI client. Required to enable login. */
  clientId: string;
  /** Loopback redirect port; default 4747 with fallback range 4747–4757. */
  redirectPort?: number;
  /**
   * Per-installation scope override. Defaults to the union of scopes
   * referenced by per-op `security` requirements pointing at the spec's
   * oauth2 scheme.
   */
  scopes?: string[];
}

export interface PluginConfig {
  // Package name written into the generated `package.json`. Defaults to a
  // kebab-case form of `info.title`.
  name?: string;
  // Override the API base URL. Falls back to `servers[0].url` from the spec
  // and finally to a `--base-url` CLI flag for end users.
  baseUrl?: string;
  // Prefix for env-var auth lookups. Defaults to a SCREAMING_SNAKE form of
  // `name` — e.g. `PET_STORE_TOKEN`, `PET_STORE_API_KEY`.
  envPrefix?: string;
  // When set AND the spec declares an `oauth2` scheme with an
  // `authorizationCode` flow, the generated CLI gets `login` / `logout`
  // subcommands that run real PKCE auth and persist the token at the
  // platform config dir. `clientId` is per-installation, never derivable
  // from the spec.
  oauth?: OAuthConfig;
}

export const CONFIG_SCHEMA = JSON.stringify({
  type: 'object',
  additionalProperties: false,
  properties: {
    name: { type: 'string', minLength: 1 },
    baseUrl: { type: 'string', minLength: 1 },
    envPrefix: { type: 'string', pattern: '^[A-Z][A-Z0-9_]*$' },
    oauth: {
      type: 'object',
      additionalProperties: false,
      required: ['clientId'],
      properties: {
        clientId: { type: 'string', minLength: 1 },
        redirectPort: { type: 'integer', minimum: 1, maximum: 65535 },
        scopes: { type: 'array', items: { type: 'string' } },
      },
    },
  },
});

export function parseConfig(raw: string): PluginConfig {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (e) {
    throwConfigInvalid(`config: invalid JSON: ${(e as Error).message}`);
  }
  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
    throwConfigInvalid('config: expected a JSON object');
  }
  const obj = parsed as Record<string, unknown>;
  // The host's CLI runs jsonschema validation against `CONFIG_SCHEMA`
  // before invoking us, but `PluginRunner` (and any other direct host
  // entry point) skips that step. Validate strictly here so plugin
  // behaviour is consistent across host paths.
  const allowed = new Set(['name', 'baseUrl', 'envPrefix', 'oauth']);
  for (const k of Object.keys(obj)) {
    if (!allowed.has(k)) throwConfigInvalid(`config: unknown field "${k}"`);
  }
  const cfg: PluginConfig = {};
  if ('name' in obj) {
    if (typeof obj['name'] !== 'string' || obj['name'].length === 0) {
      throwConfigInvalid('config.name: must be a non-empty string');
    }
    cfg.name = obj['name'];
  }
  if ('baseUrl' in obj) {
    if (typeof obj['baseUrl'] !== 'string' || obj['baseUrl'].length === 0) {
      throwConfigInvalid('config.baseUrl: must be a non-empty string');
    }
    cfg.baseUrl = obj['baseUrl'];
  }
  if ('envPrefix' in obj) {
    if (typeof obj['envPrefix'] !== 'string' || !/^[A-Z][A-Z0-9_]*$/.test(obj['envPrefix'])) {
      throwConfigInvalid('config.envPrefix: must match /^[A-Z][A-Z0-9_]*$/');
    }
    cfg.envPrefix = obj['envPrefix'];
  }
  if ('oauth' in obj) {
    cfg.oauth = parseOAuth(obj['oauth']);
  }
  return cfg;
}

function parseOAuth(raw: unknown): OAuthConfig {
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
    throwConfigInvalid('config.oauth: must be a JSON object');
  }
  const obj = raw as Record<string, unknown>;
  const allowed = new Set(['clientId', 'redirectPort', 'scopes']);
  for (const k of Object.keys(obj)) {
    if (!allowed.has(k)) throwConfigInvalid(`config.oauth: unknown field "${k}"`);
  }
  const clientId = obj['clientId'];
  if (typeof clientId !== 'string' || clientId.length === 0) {
    throwConfigInvalid('config.oauth.clientId: required, must be a non-empty string');
  }
  const out: OAuthConfig = { clientId };
  if ('redirectPort' in obj) {
    const p = obj['redirectPort'];
    if (typeof p !== 'number' || !Number.isInteger(p) || p < 1 || p > 65535) {
      throwConfigInvalid('config.oauth.redirectPort: must be an integer in [1, 65535]');
    }
    out.redirectPort = p;
  }
  if ('scopes' in obj) {
    const s = obj['scopes'];
    if (!Array.isArray(s) || !s.every((x) => typeof x === 'string')) {
      throwConfigInvalid('config.oauth.scopes: must be an array of strings');
    }
    out.scopes = s as string[];
  }
  return out;
}
