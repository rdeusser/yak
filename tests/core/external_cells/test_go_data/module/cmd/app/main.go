package main

import (
	"fmt"

	"example.com/app/greet"
	"example.com/dep"
)

func main() {
	fmt.Println(greet.Greet(dep.Name()))
}
