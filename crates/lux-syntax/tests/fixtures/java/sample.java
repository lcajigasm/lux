// Fixture exercising common Java constructs.
package sample;

public class Point {
    private final double x;
    private final double y;

    public Point(double x, double y) {
        this.x = x;
        this.y = y;
    }

    public double distance(Point other) {
        double dx = this.x - other.x;
        double dy = this.y - other.y;
        return Math.sqrt(dx * dx + dy * dy);
    }

    public static void main(String[] args) {
        Point origin = new Point(0, 0);
        Point p = new Point(3, 4);
        System.out.println("distance = " + origin.distance(p));
    }
}
