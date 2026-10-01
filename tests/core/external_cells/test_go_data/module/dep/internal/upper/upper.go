// Package upper upper-cases strings.
package upper

import "strings"

// Upper returns s in upper case.
func Upper(s string) string {
	return strings.ToUpper(s)
}
