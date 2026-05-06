package emit

import (
	"fmt"
	"strings"
)

// GoMod renders go.mod. The output server uses only the standard library,
// so there are no `require` directives.
//
// Go 1.22 is required for `http.ServeMux` method-prefixed patterns and
// `(*http.Request).PathValue`. We pin 1.22 as the floor.
func GoMod(modulePath string) string {
	var b strings.Builder
	fmt.Fprintf(&b, "module %s\n\n", modulePath)
	b.WriteString("go 1.22\n")
	return b.String()
}
