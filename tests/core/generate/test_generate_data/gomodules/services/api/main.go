package main

import (
	"fmt"

	"example.com/api/internal/store"
)

func main() {
	fmt.Println("api serves", store.Count())
}
