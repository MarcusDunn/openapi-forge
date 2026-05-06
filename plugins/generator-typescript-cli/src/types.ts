// Internal IR view. Mirrors the WIT shapes jco passes us at the boundary,
// trimmed to the fields the CLI generator actually consumes.
//
// jco's mapping rules (per its WIT-type docs):
//   - `record { foo: option<T>, bar: T }` → `{ foo?: T; bar: T }`
//   - `variant kind(payload)` → `{ tag: 'name', val: payload }`
//   - `list<T>` → `T[]`
//
// We don't import jco-generated `.d.ts` bindings; the surface is small and
// stable enough that hand-rolled types stay readable.

export interface PluginInfo {
  name: string;
  version: string;
}

export type Severity = 'error' | 'warning' | 'info' | 'hint';

export interface SpecLocation {
  pointer: string;
  file?: string;
}

export interface Diagnostic {
  severity: Severity;
  code: string;
  message: string;
  location?: SpecLocation;
  related: { message: string; location?: SpecLocation }[];
  suggestedFix?: { message: string; edits: { location: SpecLocation; replacement: string }[] };
}

export interface ApiInfo {
  title: string;
  version: string;
  description?: string;
  summary?: string;
  termsOfService?: string;
  contact?: Contact;
  licenseName?: string;
  licenseUrl?: string;
  licenseIdentifier?: string;
  extensions: [string, Value][];
}

export interface Contact {
  name?: string;
  url?: string;
  email?: string;
}

export interface Server {
  url: string;
  description?: string;
  name?: string;
  variables: [string, ServerVariable][];
  extensions: [string, Value][];
}

export interface ServerVariable {
  default: string;
  enum?: string[];
  description?: string;
  extensions: [string, Value][];
}

export type PrimitiveKind =
  | 'prim-string'
  | 'prim-integer'
  | 'prim-number'
  | 'prim-bool';

export interface PrimitiveType {
  kind: PrimitiveKind;
  constraints: PrimitiveConstraints;
}

export interface PrimitiveConstraints {
  minimum?: Value;
  maximum?: Value;
  exclusiveMinimum?: Value;
  exclusiveMaximum?: Value;
  multipleOf?: Value;
  minLength?: bigint;
  maxLength?: bigint;
  pattern?: string;
  formatExtension?: string;
  contentEncoding?: string;
  contentMediaType?: string;
  contentSchema?: TypeRef;
}

export type Value =
  | { tag: 'null' }
  | { tag: 'bool'; val: boolean }
  | { tag: 'int'; val: bigint }
  | { tag: 'float'; val: number }
  | { tag: 'string'; val: string };

export type TypeRef = string;

export interface ArrayType {
  items: TypeRef;
  constraints: { minItems?: bigint; maxItems?: bigint; uniqueItems: boolean };
}

export type AdditionalProperties =
  | { tag: 'forbidden' }
  | { tag: 'any' }
  | { tag: 'typed'; val: TypeRef };

export interface ObjectType {
  properties: Property[];
  additionalProperties: AdditionalProperties;
  constraints: { minProperties?: bigint; maxProperties?: bigint };
}

export interface Property {
  name: string;
  type: TypeRef;
  required: boolean;
  documentation?: string;
  deprecated: boolean;
  readOnly: boolean;
  writeOnly: boolean;
  default?: Value;
  extensions: [string, Value][];
}

export interface EnumStringValue {
  value: string;
  documentation?: string;
}
export interface EnumStringType {
  values: EnumStringValue[];
}

export interface EnumIntValue {
  value: bigint;
  documentation?: string;
}
export type IntKind = 'int32' | 'int64';
export interface EnumIntType {
  intKind: IntKind;
  values: EnumIntValue[];
}

export type UnionKind = 'one-of' | 'any-of';
export interface Discriminator {
  propertyName: string;
  mapping: [string, TypeRef][];
  extensions: [string, Value][];
}
export interface UnionVariant {
  type: TypeRef;
  name?: string;
}
export interface UnionType {
  variants: UnionVariant[];
  discriminator?: Discriminator;
  kind: UnionKind;
}

export type TypeDef =
  | { tag: 'primitive'; val: PrimitiveType }
  | { tag: 'object'; val: ObjectType }
  | { tag: 'array'; val: ArrayType }
  | { tag: 'enum-string'; val: EnumStringType }
  | { tag: 'enum-int'; val: EnumIntType }
  | { tag: 'union'; val: UnionType }
  | { tag: 'null' };

export interface NamedType {
  id: string;
  originalName?: string;
  documentation?: string;
  readOnly: boolean;
  writeOnly: boolean;
  externalDocs?: ExternalDocs;
  default?: Value;
  examples: [string, Example][];
  xml?: XmlObject;
  definition: TypeDef;
  extensions: [string, Value][];
  location?: SpecLocation;
}

export interface ExternalDocs {
  description?: string;
  url: string;
}

export interface Example {
  summary?: string;
  description?: string;
  value?: Value;
  externalValue?: string;
  dataValue?: Value;
  serializedValue?: string;
}

export interface XmlObject {
  name?: string;
  namespace?: string;
  prefix?: string;
  attribute: boolean;
  wrapped: boolean;
  text: boolean;
  ordered: boolean;
  extensions: [string, Value][];
}

export interface Link {
  operationRef?: string;
  operationId?: string;
  parameters: [string, Value][];
  requestBody?: Value;
  description?: string;
  server?: Server;
  extensions: [string, Value][];
}

export interface Webhook {
  name: string;
  operations: Operation[];
}

export interface Callback {
  name: string;
  expression: string;
  operationIds: string[];
  extensions: [string, Value][];
}

// jco renders WIT variants as `{ tag, val? }`. The OAS 3.2 `other(string)`
// case carries the verb verbatim; the eight standard verbs are payload-free.
export type HttpMethod =
  | { tag: 'get' }
  | { tag: 'put' }
  | { tag: 'post' }
  | { tag: 'delete' }
  | { tag: 'options' }
  | { tag: 'head' }
  | { tag: 'patch' }
  | { tag: 'trace' }
  | { tag: 'other'; val: string };
export type ParameterStyle =
  | 'matrix'
  | 'label'
  | 'form'
  | 'simple'
  | 'space-delimited'
  | 'pipe-delimited'
  | 'deep-object';

export interface Parameter {
  name: string;
  type: TypeRef;
  required: boolean;
  documentation?: string;
  deprecated: boolean;
  style?: ParameterStyle;
  explode: boolean;
  allowEmptyValue: boolean;
  allowReserved: boolean;
  examples: [string, Example][];
  extensions: [string, Value][];
  location?: SpecLocation;
}

export interface BodyContent {
  mediaType: string;
  type: TypeRef;
  encoding: [string, Encoding][];
  itemSchema?: TypeRef;
  examples: [string, Example][];
  extensions: [string, Value][];
}
export interface Encoding {
  contentType?: string;
  style?: ParameterStyle;
  explode: boolean;
  allowReserved: boolean;
  headers: [string, Header][];
  extensions: [string, Value][];
}

export interface Header {
  type: TypeRef;
  required: boolean;
  deprecated: boolean;
  documentation?: string;
  examples: [string, Example][];
  style?: ParameterStyle;
  explode: boolean;
  allowReserved: boolean;
  allowEmptyValue: boolean;
  location?: SpecLocation;
}

export interface Body {
  content: BodyContent[];
  required: boolean;
  documentation?: string;
  extensions: [string, Value][];
}

export type ResponseStatus =
  | { tag: 'explicit'; val: number }
  | { tag: 'range'; val: 1 | 2 | 3 | 4 | 5 }
  | { tag: 'default' };

export interface Response {
  status: ResponseStatus;
  content: BodyContent[];
  headers: [string, Header][];
  documentation?: string;
  links: [string, Link][];
  extensions: [string, Value][];
}

export type OAuth2FlowKind = 'implicit' | 'password' | 'client-credentials' | 'authorization-code';

export interface OAuth2Flow {
  kind: OAuth2FlowKind;
  authorizationUrl?: string;
  tokenUrl?: string;
  refreshUrl?: string;
  scopes: [string, string][];
  extensions: [string, Value][];
}

export interface OAuth2Scheme {
  flows: OAuth2Flow[];
}

export type SecuritySchemeKind =
  | { tag: 'api-key'; val: { in: 'header' | 'query' | 'cookie'; name: string } }
  | { tag: 'http-bearer'; val: { bearerFormat?: string } }
  | { tag: 'http-basic' }
  | { tag: 'mutual-tls' }
  | { tag: 'oauth2'; val: OAuth2Scheme }
  | { tag: 'open-id-connect'; val: string };

export interface SecurityScheme {
  id: string;
  description?: string;
  kind: SecuritySchemeKind;
  deprecated: boolean;
  extensions: [string, Value][];
}

export interface SecurityRequirement {
  schemeId: string;
  scopes: string[];
}

export interface Operation {
  id: string;
  originalId?: string;
  method: HttpMethod;
  pathTemplate: string;
  pathParams: Parameter[];
  queryParams: Parameter[];
  headerParams: Parameter[];
  cookieParams: Parameter[];
  querystringParams: Parameter[];
  requestBody?: Body;
  responses: Response[];
  security: SecurityRequirement[];
  tags: string[];
  documentation?: string;
  deprecated: boolean;
  extensions: [string, Value][];
  externalDocs?: ExternalDocs;
  servers: Server[];
  callbacks: Callback[];
  location?: SpecLocation;
}

export interface Ir {
  info: ApiInfo;
  jsonSchemaDialect?: string;
  selfUrl?: string;
  operations: Operation[];
  types: NamedType[];
  securitySchemes: SecurityScheme[];
  servers: Server[];
  webhooks: Webhook[];
  externalDocs?: ExternalDocs;
  tags: Tag[];
}

export interface Tag {
  name: string;
  summary?: string;
  description?: string;
  externalDocs?: ExternalDocs;
  parent?: string;
  kind?: string;
  extensions: [string, Value][];
}

// ---------- output ----------

export type FileMode = 'text' | 'binary' | 'executable';

export interface OutputFile {
  path: string;
  content: Uint8Array;
  mode: FileMode;
}

export interface GenerationOutput {
  files: OutputFile[];
  diagnostics: Diagnostic[];
}

// ---------- stage error ----------

export type ResourceKind = 'fuel' | 'memory' | 'time' | 'output-size';

export type StageError =
  | { tag: 'rejected'; val: { reason: string; diagnostics: Diagnostic[] } }
  | { tag: 'plugin-bug'; val: string }
  | { tag: 'config-invalid'; val: string }
  | { tag: 'resource-exceeded'; val: ResourceKind };
