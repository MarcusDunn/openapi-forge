// Package emit renders the Go server scaffold from an IR.
//
// Pure functions only — no wit-bindgen types in this package.
package emit

import (
	"strings"
	"unicode"
)

// goKeywords is the set of Go reserved words. A sanitized identifier that
// would collide gets a trailing underscore.
var goKeywords = map[string]struct{}{
	"break": {}, "case": {}, "chan": {}, "const": {}, "continue": {},
	"default": {}, "defer": {}, "else": {}, "fallthrough": {}, "for": {},
	"func": {}, "go": {}, "goto": {}, "if": {}, "import": {},
	"interface": {}, "map": {}, "package": {}, "range": {}, "return": {},
	"select": {}, "struct": {}, "switch": {}, "type": {}, "var": {},
	// predeclared identifiers we don't want to shadow either
	"any": {}, "bool": {}, "byte": {}, "comparable": {}, "complex64": {},
	"complex128": {}, "error": {}, "float32": {}, "float64": {},
	"int": {}, "int8": {}, "int16": {}, "int32": {}, "int64": {},
	"rune": {}, "string": {}, "uint": {}, "uint8": {}, "uint16": {},
	"uint32": {}, "uint64": {}, "uintptr": {},
	"true": {}, "false": {}, "iota": {}, "nil":     {},
	"append": {}, "cap": {}, "clear": {}, "close": {}, "complex": {},
	"copy": {}, "delete": {}, "imag": {}, "len": {}, "make": {},
	"max": {}, "min": {}, "new": {}, "panic": {}, "print": {},
	"println": {}, "real": {}, "recover": {},
}

// PascalCase converts an arbitrary identifier-shaped string into a Go
// PascalCase identifier. Splits on non-alphanumerics and at lower→upper
// camel transitions.
func PascalCase(raw string) string {
	parts := splitWords(raw)
	var b strings.Builder
	for _, p := range parts {
		if p == "" {
			continue
		}
		runes := []rune(strings.ToLower(p))
		runes[0] = unicode.ToUpper(runes[0])
		b.WriteString(string(runes))
	}
	out := b.String()
	if out == "" {
		out = "Op"
	}
	// PascalCase identifiers are exported and won't normally collide with
	// keywords (which are lowercase), but a single-letter input like "if"
	// becomes "If" — fine — so we only worry about the rare case where the
	// PascalCase output exactly matches a predeclared *type* name like
	// "Error". That's actually a user-visible name; leave it alone and let
	// the consumer alias if needed.
	return out
}

// LowerCamel converts to lowerCamelCase, used for parameter / variable names
// where the keyword check matters.
func LowerCamel(raw string) string {
	parts := splitWords(raw)
	var b strings.Builder
	for i, p := range parts {
		if p == "" {
			continue
		}
		if i == 0 {
			b.WriteString(strings.ToLower(p))
			continue
		}
		runes := []rune(strings.ToLower(p))
		runes[0] = unicode.ToUpper(runes[0])
		b.WriteString(string(runes))
	}
	out := b.String()
	if out == "" {
		out = "v"
	}
	if _, isKw := goKeywords[out]; isKw {
		out += "_"
	}
	return out
}

// splitWords breaks "fooBar_baz-qux" → ["foo","Bar","baz","qux"].
func splitWords(s string) []string {
	var out []string
	var cur strings.Builder
	flush := func() {
		if cur.Len() > 0 {
			out = append(out, cur.String())
			cur.Reset()
		}
	}
	var prev rune
	for i, r := range s {
		switch {
		case !isWordChar(r):
			flush()
		case i > 0 && unicode.IsLower(prev) && unicode.IsUpper(r):
			flush()
			cur.WriteRune(r)
		default:
			cur.WriteRune(r)
		}
		prev = r
	}
	flush()
	return out
}

func isWordChar(r rune) bool {
	return unicode.IsLetter(r) || unicode.IsDigit(r)
}

// PackageName converts a module path's last segment into a valid Go package
// identifier. "github.com/example/petstore" → "petstore".
func PackageName(modulePath string) string {
	last := modulePath
	if i := strings.LastIndex(last, "/"); i >= 0 {
		last = last[i+1:]
	}
	var b strings.Builder
	for _, r := range last {
		switch {
		case unicode.IsLetter(r):
			b.WriteRune(unicode.ToLower(r))
		case unicode.IsDigit(r) && b.Len() > 0:
			b.WriteRune(r)
		}
	}
	out := b.String()
	if out == "" {
		out = "server"
	}
	if _, isKw := goKeywords[out]; isKw {
		out += "_"
	}
	return out
}
