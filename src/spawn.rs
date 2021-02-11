use crate::actor::Actor;
use crate::unify::Case;

fn spawn<Unified, Specific, A>(actor: A, name: String) where 
 A: Actor<Unified, Specific>,
 Unified: Clone + Case<Specific> {

}

