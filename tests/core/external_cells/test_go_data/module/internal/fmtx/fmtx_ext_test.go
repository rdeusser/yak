package fmtx_test

import (
	"testing"

	"example.com/app/internal/fmtx"
	"example.com/app/internal/report"
)

// TestReport links `report`, which imports `fmtx`, so the test and `report` share one `fmtx`.
func TestReport(t *testing.T) {
	if got := report.Line("ok"); got != fmtx.Bracket("ok") {
		t.Fatalf("Line = %q", got)
	}
}
