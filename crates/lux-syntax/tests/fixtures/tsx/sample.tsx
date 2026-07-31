// Fixture exercising common TSX constructs.
import React from "react";

interface GreetingProps {
  name: string;
}

function Greeting({ name }: GreetingProps) {
  return (
    <div className="greeting">
      <span>hello, {name}</span>
    </div>
  );
}

export default Greeting;
