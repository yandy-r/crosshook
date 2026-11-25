//! Tree walk that collects every focus target and its bounds, in tree order.
//! Text inputs report through iced's own `focusable` hook; our wrappers report the same way.

use iced::Rectangle;
use iced::advanced::widget::Id;
use iced::advanced::widget::operation::{Focusable, Operation, Outcome};

use super::target::{PREFIX, Target};

pub type Items = Vec<(Target, Rectangle)>;

#[derive(Default)]
pub struct Collect {
    pub items: Items,
}

impl Collect {
    fn record(&mut self, id: Option<&Id>, bounds: Rectangle) {
        let Some(id) = id else { return };
        // `Id` has no public accessor for its string at this rev; its Debug form embeds it.
        let debug = format!("{id:?}");
        let Some(start) = debug.find(PREFIX) else {
            return;
        };
        let key: String = debug[start + PREFIX.len()..]
            .chars()
            .take_while(|c| *c != '"')
            .collect();
        if let Some(target) = Target::parse(&key)
            && !self.items.iter().any(|(t, _)| *t == target)
        {
            self.items.push((target, bounds));
        }
    }
}

impl Operation<Items> for Collect {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Items>)) {
        operate(self);
    }

    fn focusable(&mut self, id: Option<&Id>, bounds: Rectangle, _state: &mut dyn Focusable) {
        self.record(id, bounds);
    }

    fn finish(&self) -> Outcome<Items> {
        Outcome::Some(self.items.clone())
    }
}

/// `Operation<()>` adapter for the headless simulator, which only accepts unit operations.
#[cfg(test)]
pub struct CollectUnit<'a>(pub &'a mut Collect);

#[cfg(test)]
impl Operation for CollectUnit<'_> {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn focusable(&mut self, id: Option<&Id>, bounds: Rectangle, _state: &mut dyn Focusable) {
        self.0.record(id, bounds);
    }
}
