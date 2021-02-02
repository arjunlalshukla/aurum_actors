
trait LocalActor<T> {
  fn recv(&mut self, msg: T);
}

trait RemoteActor<'a, T: Serialize + Deserialize<'a>> {
  fn recv(&mut self, msg: T);
}
