// Fixture exercising common C++ constructs.
#include <iostream>
#include <vector>

class Point {
public:
    Point(double x, double y) : x_(x), y_(y) {}

    double distance(const Point& other) const {
        double dx = x_ - other.x_;
        double dy = y_ - other.y_;
        return dx * dx + dy * dy;
    }

private:
    double x_;
    double y_;
};

int main() {
    std::vector<Point> points{{0, 0}, {3, 4}};
    std::cout << "distance = " << points[0].distance(points[1]) << std::endl;
    return 0;
}
