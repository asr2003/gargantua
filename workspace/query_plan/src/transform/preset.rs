use std::rc::Rc;

use blueprint::Index;
use valid::Transform;

use super::{Enrich, Minify};
use crate::QueryPlan;

pub struct Preset<A> {
    index: Rc<Index>,
    _marker: std::marker::PhantomData<A>,
}

impl<A> Preset<A> {
    #[allow(dead_code)]
    pub fn new(index: Rc<Index>) -> Self {
        Self { index, _marker: std::marker::PhantomData }
    }
}

impl<A: Clone> Transform for Preset<A> {
    type Value = QueryPlan<A>;

    type Error = String;

    fn transform(&self, input: Self::Value) -> valid::Valid<Self::Value, String> {
        Minify::new()
            .map_err(|e| e.to_string())
            .pipe(Enrich::new(self.index.clone()))
            .transform(input)
    }
}
