// Fixture exercising common Kotlin constructs.
package sample

data class Point(val x: Double, val y: Double) {
    fun distance(other: Point): Double {
        val dx = x - other.x
        val dy = y - other.y
        return Math.sqrt(dx * dx + dy * dy)
    }
}

fun main() {
    val origin = Point(0.0, 0.0)
    val p = Point(3.0, 4.0)
    println("distance = ${origin.distance(p)}")
}
