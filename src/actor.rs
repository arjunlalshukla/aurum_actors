use crossbeam_channel::Sender;
use serde::Serialize;
use serde::de::DeserializeOwned;

type LocalRef<T> = Box<dyn Fn(T) -> bool>;

trait Actor<Msg: Serialize + DeserializeOwned> {
  fn pre_start(&mut self) {}
  fn recv(&mut self, ctx: ActorContext<Msg>, msg: Msg);
  fn post_stop(&mut self) {}
}

pub struct ActorContext<Specific> {
  tx: Sender<Specific>
}
impl<Specific> ActorContext<Specific> {
  fn local_ref<T>(&self) -> LocalRef<T>
    where Specific: Translatable<T> + 'static {
    
    let sender = self.tx.clone();
    Box::new(move |x: T|
      sender.send(<Specific as Translatable<T>>::translate(x)).is_ok()
    )
  }
}

pub trait Translatable<T> {
  fn translate(item: T) -> Self;
}