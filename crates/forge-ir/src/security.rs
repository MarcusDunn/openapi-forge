//! Security schemes and requirements.

use serde::{Deserialize, Serialize};

use crate::value::ValueRef;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecurityScheme {
    pub id: String,
    pub kind: SecuritySchemeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    /// OAS 3.2 `deprecated` flag. Generators should surface this as a
    /// deprecation hint (doc comment, `@deprecated`, etc.) so consumers
    /// migrate off the scheme. Defaults to `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub deprecated: bool,
    /// `x-*` extensions declared on the security scheme object. Compound
    /// extensions drop with `parser/W-EXTENSION-DROPPED`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<(String, ValueRef)>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SecuritySchemeKind {
    ApiKey(ApiKeyScheme),
    HttpBasic,
    HttpBearer {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bearer_format: Option<String>,
    },
    /// OAS 3.0+ `mutualTLS` — client-cert auth (mTLS). The IR carries
    /// the declaration; certificate provisioning is out of scope and
    /// left to the consumer's transport configuration.
    MutualTls,
    Oauth2(OAuth2Scheme),
    OpenIdConnect {
        url: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiKeyScheme {
    pub name: String,
    pub location: ApiKeyLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApiKeyLocation {
    Header,
    Query,
    Cookie,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuth2Scheme {
    pub flows: Vec<OAuth2Flow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuth2Flow {
    pub kind: OAuth2FlowKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<(String, String)>,
    /// `x-*` extensions declared on the OAuth2 flow object. Compound
    /// extensions drop with `parser/W-EXTENSION-DROPPED`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<(String, ValueRef)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OAuth2FlowKind {
    Implicit,
    Password,
    ClientCredentials,
    AuthorizationCode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecurityRequirement {
    pub scheme_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
}
