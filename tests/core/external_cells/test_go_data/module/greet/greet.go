// Package greet greets with a message that it embeds.
package greet

import (
	_ "embed"
	"strings"
)

//go:embed message.txt
var message string

// Greet returns the greeting for name.
func Greet(name string) string {
	return strings.ReplaceAll(strings.TrimSpace(message), "NAME", name)
}
