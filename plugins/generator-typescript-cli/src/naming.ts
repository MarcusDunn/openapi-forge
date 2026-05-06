// Identifier sanitization helpers. Mirrors
// generator-typescript-fetch/src/naming.rs's ts_ident / ts_type_name; adds
// kebab-case (CLI subcommand names) and SCREAMING_SNAKE (env var names).

const TS_RESERVED = new Set([
  'break', 'case', 'catch', 'class', 'const', 'continue', 'debugger', 'default',
  'delete', 'do', 'else', 'enum', 'export', 'extends', 'false', 'finally', 'for',
  'function', 'if', 'import', 'in', 'instanceof', 'new', 'null', 'return',
  'super', 'switch', 'this', 'throw', 'true', 'try', 'typeof', 'var', 'void',
  'while', 'with', 'as', 'implements', 'interface', 'let', 'package', 'private',
  'protected', 'public', 'static', 'yield', 'any', 'boolean', 'constructor',
  'declare', 'get', 'module', 'require', 'number', 'set', 'string', 'symbol',
  'type', 'from', 'of', 'async', 'await', 'object', 'undefined',
]);

function isWordChar(ch: string): boolean {
  return /[A-Za-z0-9]/.test(ch);
}

function isUpper(ch: string): boolean {
  return ch >= 'A' && ch <= 'Z';
}

function isLower(ch: string): boolean {
  return ch >= 'a' && ch <= 'z';
}

// Split "fooBar_baz-qux" → ["foo","Bar","baz","qux"].
export function splitWords(s: string): string[] {
  const out: string[] = [];
  let cur = '';
  let prev = '';
  for (let i = 0; i < s.length; i++) {
    const ch = s[i]!;
    if (!isWordChar(ch)) {
      if (cur) { out.push(cur); cur = ''; }
    } else if (i > 0 && isLower(prev) && isUpper(ch)) {
      if (cur) { out.push(cur); cur = ''; }
      cur += ch;
    } else {
      cur += ch;
    }
    prev = ch;
  }
  if (cur) out.push(cur);
  return out;
}

function capitalize(s: string): string {
  if (!s) return s;
  return s[0]!.toUpperCase() + s.slice(1).toLowerCase();
}

// PascalCase. For type names: `petStoreError` → `PetStoreError`.
export function pascalCase(raw: string): string {
  const parts = splitWords(raw);
  if (parts.length === 0) return 'T';
  return parts.map(capitalize).join('');
}

// camelCase. For variable / parameter names: `pet_store_id` → `petStoreId`.
// Reserved-word collisions get a trailing underscore, matching the existing
// TS-fetch generator.
export function camelCase(raw: string): string {
  const parts = splitWords(raw);
  if (parts.length === 0) return 'v';
  let out = parts[0]!.toLowerCase();
  for (let i = 1; i < parts.length; i++) out += capitalize(parts[i]!);
  if (/^[0-9]/.test(out)) out = '_' + out;
  if (TS_RESERVED.has(out)) out += '_';
  return out;
}

// kebab-case. For CLI subcommand names: `listIssuesForRepo` → `list-issues-for-repo`.
export function kebabCase(raw: string): string {
  const parts = splitWords(raw);
  if (parts.length === 0) return 'cmd';
  return parts.map((p) => p.toLowerCase()).join('-');
}

// SCREAMING_SNAKE. For env var names: `petStoreApi` → `PET_STORE_API`.
export function screamingSnake(raw: string): string {
  const parts = splitWords(raw);
  if (parts.length === 0) return 'V';
  return parts.map((p) => p.toUpperCase()).join('_');
}

// CLI binary name. Like kebab but strips redundant `-cli` / `-api` suffixes
// for tidy output. e.g. `petStore-api` → `pet-store`.
export function binName(raw: string): string {
  const k = kebabCase(raw);
  return k.replace(/-(api|cli|client)$/i, '') || k;
}
