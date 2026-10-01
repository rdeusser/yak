package greet

import (
	"os"
	"testing"
)

// TestData reads a file of the package's directory and a file that the build file declares.
func TestData(t *testing.T) {
	for path, want := range map[string]string{
		"testdata/names.txt":  "Ada\n",
		"../shared/words.txt": "hello\n",
	} {
		got, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		if string(got) != want {
			t.Fatalf("%s = %q, want %q", path, got, want)
		}
	}
}
