package greet

import "testing"

func TestMessage(t *testing.T) {
	if message != "Hello, NAME!\n" {
		t.Fatalf("message = %q", message)
	}
}
