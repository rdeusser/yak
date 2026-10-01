package calc

import "testing"

func TestDouble(t *testing.T) {
	if double(2) != 4 {
		t.Fatal("double(2) != 4")
	}
}
