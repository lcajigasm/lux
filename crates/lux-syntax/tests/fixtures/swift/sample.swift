// Fixture exercising common Swift constructs.
import Foundation

struct Point {
    let x: Double
    let y: Double

    func distance(to other: Point) -> Double {
        let dx = x - other.x
        let dy = y - other.y
        return (dx * dx + dy * dy).squareRoot()
    }
}

let origin = Point(x: 0, y: 0)
let p = Point(x: 3, y: 4)
print("distance = \(origin.distance(to: p))")
