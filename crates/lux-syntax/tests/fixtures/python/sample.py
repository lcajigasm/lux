# Fixture exercising common Python constructs.
from dataclasses import dataclass


@dataclass
class Point:
    x: float
    y: float

    def distance(self, other: "Point") -> float:
        return ((self.x - other.x) ** 2 + (self.y - other.y) ** 2) ** 0.5


def main():
    origin = Point(0.0, 0.0)
    p = Point(3.0, 4.0)
    print(f"distance = {origin.distance(p)}")


if __name__ == "__main__":
    main()
