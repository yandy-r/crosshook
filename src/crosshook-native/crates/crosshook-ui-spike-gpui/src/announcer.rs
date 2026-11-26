//! M0-09 probe (YAN-877): one live-region announcer for the gpui spike.
//!
//! Two always-present, off-screen-sized AccessKit nodes: `Role::Status` with `Live::Polite`
//! and `Role::Alert` with `Live::Assertive`. Each message becomes a synthetic `Role::Label`
//! child whose id changes per message, so the adapter reports a new child of a live region
//! (the AT-SPI `children-changed` path Orca speaks for live regions) without moving focus.

use gpui::accesskit::{Live, Node, Role};
use gpui::{
    AnyElement, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, div,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Politeness {
    Polite,
    Assertive,
}

#[derive(Default)]
pub struct Announcer {
    polite: Option<(u64, SharedString)>,
    assertive: Option<(u64, SharedString)>,
    seq: u64,
}

impl Announcer {
    pub fn announce(&mut self, politeness: Politeness, text: impl Into<SharedString>) {
        self.seq += 1;
        let entry = Some((self.seq, text.into()));
        match politeness {
            Politeness::Polite => self.polite = entry,
            Politeness::Assertive => self.assertive = entry,
        }
    }

    /// Both live regions. Visually empty (zero size); present in every frame so the regions
    /// exist before the first message, as ARIA requires for reliable announcements.
    pub fn render(&self) -> AnyElement {
        div()
            .size_0()
            .overflow_hidden()
            .child(region(
                "announcer-polite",
                Role::Status,
                Live::Polite,
                self.polite.clone(),
            ))
            .child(region(
                "announcer-assertive",
                Role::Alert,
                Live::Assertive,
                self.assertive.clone(),
            ))
            .into_any_element()
    }
}

fn region(
    id: &'static str,
    role: Role,
    live: Live,
    message: Option<(u64, SharedString)>,
) -> impl IntoElement {
    div()
        .id(id)
        .role(role)
        .a11y_synthetic_children(move |builder| {
            builder.parent_node().set_live(live);
            if let Some((seq, text)) = message {
                let mut label = Node::new(Role::Label);
                label.set_value(text.to_string());
                let child = builder.synthetic_node_id(seq as usize);
                builder.push_child(child, label);
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_announcement_gets_a_new_sequence_number() {
        let mut a = Announcer::default();
        a.announce(Politeness::Polite, "Launch started");
        a.announce(Politeness::Polite, "Launch started");
        a.announce(Politeness::Assertive, "Save failed");
        assert_eq!(a.polite.as_ref().map(|p| p.0), Some(2));
        assert_eq!(a.assertive.as_ref().map(|p| p.0), Some(3));
    }
}
