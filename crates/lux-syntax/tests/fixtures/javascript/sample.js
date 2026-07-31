// Fixture exercising common JavaScript constructs.
class Counter {
  #count = 0;

  increment() {
    this.#count += 1;
    return this.#count;
  }
}

function makeCounter() {
  const counter = new Counter();
  return () => counter.increment();
}

const next = makeCounter();
console.log(next(), next());
