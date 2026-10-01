package calc_test

import (
	"testing"

	"calc"
)

func TestAdd(t *testing.T) {
	if calc.Add(2, 3) != 5 {
		t.Fatal("Add(2, 3) != 5")
	}
}

// TestExported uses a name that only the package's own test files declare.
func TestExported(t *testing.T) {
	if calc.Double(3) != 6 {
		t.Fatal("Double(3) != 6")
	}
}
