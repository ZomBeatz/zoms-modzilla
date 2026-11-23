//! Load order rules: "load this mod after that one", written by the person on a profile. They're
//! kept whenever the profile is saved or put into the game, moving as little as possible.

use crate::deploy::ProfileMod;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    /// This mod loads…
    pub mod_id: u64,
    /// …after this one (and so wins its clashes).
    pub after: u64,
}

/// Put `mods` in an order that keeps `rules`, moving a mod down to just after the one it must
/// follow. Rules about mods not in the list are skipped. Gives the rules that can't all hold
/// (they go round in a circle), which are left as they are.
pub fn keep(mods: &mut Vec<ProfileMod>, rules: &[Rule]) -> Vec<Rule> {
    let pos = |mods: &[ProfileMod], id: u64| mods.iter().position(|m| m.mod_id == id);
    for _ in 0..=rules.len() {
        let mut moved = false;
        for r in rules {
            let (Some(a), Some(b)) = (pos(mods, r.mod_id), pos(mods, r.after)) else { continue };
            if a < b {
                let m = mods.remove(a);
                // b moved up by one when a came out above it.
                mods.insert(b, m);
                moved = true;
            }
        }
        if !moved {
            return vec![];
        }
    }
    rules.iter().filter(|r| matches!((pos(mods, r.mod_id), pos(mods, r.after)), (Some(a), Some(b)) if a < b)).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(ids: &[u64]) -> Vec<ProfileMod> {
        ids.iter().map(|&m| ProfileMod { mod_id: m, file_id: m * 10, enabled: true }).collect()
    }
    fn ids(m: &[ProfileMod]) -> Vec<u64> {
        m.iter().map(|m| m.mod_id).collect()
    }

    #[test]
    fn moves_as_little_as_needed() {
        let mut m = list(&[1, 2, 3, 4]);
        assert!(keep(&mut m, &[Rule { mod_id: 1, after: 3 }]).is_empty());
        assert_eq!(ids(&m), vec![2, 3, 1, 4]);
        // Chains settle.
        let mut m = list(&[1, 2, 3]);
        keep(&mut m, &[Rule { mod_id: 1, after: 2 }, Rule { mod_id: 2, after: 3 }]);
        assert_eq!(ids(&m), vec![3, 2, 1]);
        // Missing mods are skipped.
        let mut m = list(&[1, 2]);
        assert!(keep(&mut m, &[Rule { mod_id: 1, after: 99 }]).is_empty());
        assert_eq!(ids(&m), vec![1, 2]);
    }

    #[test]
    fn reports_circles() {
        let mut m = list(&[1, 2]);
        let bad = keep(&mut m, &[Rule { mod_id: 1, after: 2 }, Rule { mod_id: 2, after: 1 }]);
        assert!(!bad.is_empty());
    }
}
