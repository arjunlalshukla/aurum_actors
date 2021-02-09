trait Foo {
  fn hello() -> String;
  fn world() -> String;
}

struct Bar {}
impl Foo for Bar {
  fn hello() -> String {
    String::from("hello")
  }
  
  fn world() -> String {
    String::from("world")
  }
}

fn main() {

}