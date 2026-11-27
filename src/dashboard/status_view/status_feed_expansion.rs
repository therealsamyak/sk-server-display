//! Feed expansions: the expanded feed cards, at most one in each expansion
//! group. A Services card and a Devices card can be open at the same time.

use crate::metrics::FeedExpansionGroup;

/// Expanded feed cards. Opening a card closes the other card of its group.
pub(super) struct FeedExpansions<T> {
    cards: Vec<ExpandedFeedCard<T>>,
}

struct ExpandedFeedCard<T> {
    index: usize,
    group: FeedExpansionGroup,
    session: T,
}

impl<T> Default for FeedExpansions<T> {
    fn default() -> Self {
        Self { cards: Vec::new() }
    }
}

impl<T> FeedExpansions<T> {
    /// Expand a feed card and collapse the open card of the same group.
    pub(super) fn expand_feed_card(&mut self, index: usize, group: FeedExpansionGroup, session: T) {
        self.cards
            .retain(|card| card.group != group && card.index != index);
        self.cards.push(ExpandedFeedCard {
            index,
            group,
            session,
        });
    }

    /// Collapse one expanded feed card.
    pub(super) fn collapse_feed_card(&mut self, index: usize) {
        self.cards.retain(|card| card.index != index);
    }

    /// Collapse every expanded feed card.
    pub(super) fn collapse_all_feed_cards(&mut self) {
        self.cards.clear();
    }

    /// `true` when no feed card is expanded.
    pub(super) fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Session of the expanded card for a feed, if it is open.
    pub(super) fn expanded_session(&self, index: usize) -> Option<&T> {
        self.cards
            .iter()
            .find(|card| card.index == index)
            .map(|card| &card.session)
    }

    /// Sessions of all expanded cards.
    pub(super) fn sessions(&self) -> impl Iterator<Item = &T> {
        self.cards.iter().map(|card| &card.session)
    }

    /// Sessions of all expanded cards, for changes.
    pub(super) fn sessions_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.cards.iter_mut().map(|card| &mut card.session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expanded(expansions: &FeedExpansions<&'static str>) -> Vec<&'static str> {
        expansions.sessions().copied().collect()
    }

    #[test]
    fn services_and_devices_expand_together() {
        let mut expansions = FeedExpansions::default();
        expansions.expand_feed_card(4, FeedExpansionGroup::Services, "service");
        expansions.expand_feed_card(0, FeedExpansionGroup::Devices, "cachyos");
        assert_eq!(expanded(&expansions), ["service", "cachyos"]);
        assert_eq!(expansions.expanded_session(4), Some(&"service"));
        assert_eq!(expansions.expanded_session(0), Some(&"cachyos"));
    }

    #[test]
    fn expanding_a_device_collapses_the_other_device() {
        let mut expansions = FeedExpansions::default();
        expansions.expand_feed_card(4, FeedExpansionGroup::Services, "service");
        expansions.expand_feed_card(0, FeedExpansionGroup::Devices, "cachyos");
        expansions.expand_feed_card(1, FeedExpansionGroup::Devices, "laptop");
        assert_eq!(expanded(&expansions), ["service", "laptop"]);
        assert_eq!(expansions.expanded_session(0), None);
    }

    #[test]
    fn expanding_a_service_collapses_the_other_service() {
        let mut expansions = FeedExpansions::default();
        expansions.expand_feed_card(0, FeedExpansionGroup::Devices, "cachyos");
        expansions.expand_feed_card(4, FeedExpansionGroup::Services, "first");
        expansions.expand_feed_card(5, FeedExpansionGroup::Services, "second");
        assert_eq!(expanded(&expansions), ["cachyos", "second"]);
    }

    #[test]
    fn collapsing_one_card_keeps_the_other_group() {
        let mut expansions = FeedExpansions::default();
        expansions.expand_feed_card(0, FeedExpansionGroup::Devices, "cachyos");
        expansions.expand_feed_card(4, FeedExpansionGroup::Services, "service");
        expansions.collapse_feed_card(0);
        assert_eq!(expanded(&expansions), ["service"]);
        expansions.collapse_all_feed_cards();
        assert!(expansions.is_empty());
    }
}
