package greet_test

import (
	"testing"

	"example.com/app/greet"
	"example.com/dep"
)

func TestGreet(t *testing.T) {
	got := greet.Greet(dep.Name())
	if got != "Hello, DEP v1.0.0!" {
		t.Fatalf("Greet = %q", got)
	}
}
