trait Foo {
  const VARIANT: Self;
}

enum Bar {
  Hello,
  World
}
impl Foo for Bar {
  const VARIANT: Bar = Bar::Hello;
}

fn main() {
}