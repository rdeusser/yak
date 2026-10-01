// Package report writes report lines.
package report

import "example.com/app/internal/fmtx"

// Line returns the report line for s.
func Line(s string) string {
	return fmtx.Bracket(s)
}
