// Package dep is a third-party package that the test serves from a module proxy.
package dep

import (
	_ "embed"
	"strings"

	"example.com/dep/internal/upper"
)

//go:embed version.txt
var version string

// Name returns the package's name and version.
func Name() string {
	return upper.Upper("dep") + " " + strings.TrimSpace(version)
}
