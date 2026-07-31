/* Fixture exercising common C constructs. */
#include <stdio.h>

typedef struct {
    double x;
    double y;
} Point;

double distance(Point a, Point b) {
    double dx = a.x - b.x;
    double dy = a.y - b.y;
    return dx * dx + dy * dy;
}

int main(void) {
    Point origin = {0, 0};
    Point p = {3, 4};
    printf("distance = %f\n", distance(origin, p));
    return 0;
}
