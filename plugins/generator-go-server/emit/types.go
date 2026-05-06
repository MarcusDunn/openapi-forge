package emit

// Operation is the minimum slice of a forge IR operation a server scaffold
// needs. The wit-bindgen Go types are kept out of this package so renderers
// stay testable with plain Go values.
type Operation struct {
	ID         string
	Method     string // upper-case: GET / POST / ...
	Path       string // path-template, e.g. "/pets/{petId}"
	PathParams []Parameter
	Doc        string
}

// Parameter — name + Go type for path parameters. Server scaffold only
// decodes path params; query/header are left as `r.URL.Query()` for the
// implementer.
type Parameter struct {
	Name   string // sanitized, lowerCamelCase
	GoType string // "string", "int32", "int64", "float64", ...
}

// Spec is the renderer-facing IR view.
type Spec struct {
	Title       string
	Version     string
	Description string
	Operations  []Operation
}
