// Orchestrate emission. Walks the IR + plugin config, emits each file
// once. Returns the file list and any diagnostics for the host.

import type { Ir, Diagnostic, OutputFile, GenerationOutput } from '../types.js';
import type { PluginConfig } from '../config.js';
import { binName as deriveBinName, kebabCase, screamingSnake } from '../naming.js';
import { renderModels } from './models.js';
import { renderClient } from './client.js';
import { renderCli } from './cli.js';
import { renderAuth, findAuthCodeFlow, inferScopes, type OAuthEmitInputs } from './auth.js';
import { renderReadme } from './readme.js';
import { packageJson, tsconfig, binShim } from './package.js';
import { RUNTIME_TS, FORMAT_TS } from './static_files.js';

export function emitAll(ir: Ir, config: PluginConfig): GenerationOutput {
  const diagnostics: Diagnostic[] = [];

  const packageName = config.name ?? `${kebabCase(ir.info.title)}-cli`;
  const binName = deriveBinName(config.name ?? ir.info.title);
  const envPrefix = config.envPrefix ?? screamingSnake(binName);
  const baseUrl = config.baseUrl ?? ir.servers[0]?.url ?? 'http://localhost';
  const version = coerceSemver(ir.info.version);
  const description = ir.info.description ?? ir.info.title;

  // Decide whether the login flow is enabled. Two preconditions:
  //   1. The spec declares an `oauth2` scheme with an authorization-code
  //      flow (carried in `ir.securitySchemes`).
  //   2. Plugin config supplies a `clientId` — required for any public
  //      OAuth client and never derivable from the spec.
  // If only one is satisfied, emit a warning so the user knows why login
  // didn't show up.
  const authCodeFlow = findAuthCodeFlow(ir.securitySchemes);
  let oauth: OAuthEmitInputs | undefined;
  if (authCodeFlow && config.oauth?.clientId) {
    const scopes = config.oauth.scopes && config.oauth.scopes.length > 0
      ? config.oauth.scopes
      : inferScopes(ir, ir.securitySchemes);
    oauth = {
      clientId: config.oauth.clientId,
      redirectPort: config.oauth.redirectPort ?? 4747,
      scopes,
      authorizationUrl: authCodeFlow.authorizationUrl ?? '',
      tokenUrl: authCodeFlow.tokenUrl ?? '',
      refreshUrl: authCodeFlow.refreshUrl ?? authCodeFlow.tokenUrl ?? '',
      binName,
    };
  } else if (authCodeFlow && !config.oauth?.clientId) {
    diagnostics.push({
      severity: 'info',
      code: 'generator-typescript-cli/I-OAUTH-LOGIN-NOT-ENABLED',
      message:
        'spec declares an oauth2 authorizationCode flow but plugin config lacks `oauth.clientId`; login/logout subcommands skipped (set config.oauth.clientId to enable)',
      related: [],
    });
  } else if (config.oauth?.clientId && !authCodeFlow) {
    diagnostics.push({
      severity: 'warning',
      code: 'generator-typescript-cli/W-OAUTH-CLIENT-ID-UNUSED',
      message:
        'plugin config supplies `oauth.clientId` but the spec declares no oauth2 authorizationCode flow; ignoring',
      related: [],
    });
  }

  // Diagnostics for unsupported security schemes (anything that's neither
  // one of the simple kinds nor an oauth2 flow we just enabled).
  for (const s of ir.securitySchemes) {
    const isSupportedSimple = s.kind.tag === 'api-key' || s.kind.tag === 'http-bearer' || s.kind.tag === 'http-basic';
    const isHandledOauth = oauth && s.kind.tag === 'oauth2';
    if (isSupportedSimple || isHandledOauth) continue;
    diagnostics.push({
      severity: 'warning',
      code: 'generator-typescript-cli/W-SECURITY-SCHEME-SKIPPED',
      message: `security scheme "${s.id}" uses an unsupported kind; CLI will not inject auth for operations requiring it`,
      related: [],
    });
  }

  const files: OutputFile[] = [
    file('package.json', packageJson({ packageName, binName, version, description })),
    file('tsconfig.json', tsconfig()),
    file('README.md', renderReadme(ir, binName, envPrefix, !!oauth)),
    file(`bin/${binName}.js`, binShim()),
    file('src/runtime.ts', RUNTIME_TS),
    file('src/format.ts', FORMAT_TS),
    file('src/models.ts', renderModels(ir)),
    file('src/auth.ts', renderAuth(ir, { envPrefix, schemes: ir.securitySchemes, oauth })),
    file('src/client.ts', renderClient(ir)),
    file('src/cli.ts', renderCli(ir, { binName, defaultBaseUrl: baseUrl, description, oauthEnabled: !!oauth })),
    file('src/index.ts', `export { ApiClient } from "./client.js";\nexport * from "./models.js";\nexport { ApiError } from "./runtime.js";\n`),
  ];

  return { files, diagnostics };
}

function file(path: string, content: string): OutputFile {
  return { path, content: encode(content), mode: 'text' };
}

// `TextEncoder` is a Web API standard available in StarlingMonkey (the JS
// runtime jco embeds), but the TS lib config we use doesn't include
// "dom". Declare the bare slice we need.
declare const TextEncoder: { new (): { encode(s: string): Uint8Array } };

const ENCODER = new TextEncoder();
function encode(s: string): Uint8Array {
  return ENCODER.encode(s);
}

// Ensure the version we put in package.json is a valid SemVer. Many specs
// use freeform versions (`2026.04.0`, `v1`, `1`); npm rejects those. We
// coerce to a permissive `<num>.<num>.<num>` form.
function coerceSemver(raw: string): string {
  const m = /^v?(\d+)(?:\.(\d+))?(?:\.(\d+))?/.exec(raw);
  if (!m) return '0.0.0';
  const [, a = '0', b = '0', c = '0'] = m;
  return `${a}.${b}.${c}`;
}
