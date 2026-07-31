// Fixture exercising common Go constructs.
package main

import "fmt"

type Point struct {
	X, Y float64
}

func (p Point) Distance(other Point) float64 {
	dx := p.X - other.X
	dy := p.Y - other.Y
	return dx*dx + dy*dy
}

func main() {
	origin := Point{X: 0, Y: 0}
	p := Point{X: 3, Y: 4}
	fmt.Println("distance =", origin.Distance(p))
}
