package calc

// Add returns the sum of a and b.
func Add(a, b int) int {
	return double(a+b) / 2
}

func double(n int) int {
	return 2 * n
}
