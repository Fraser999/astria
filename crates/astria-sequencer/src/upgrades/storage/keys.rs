use astria_core::upgrades::v1::Change;

use super::super::Upgrade;

pub(in crate::upgrades) fn change(upgrade_name: &str, change_name: &str) -> String {
    format!("upgrades/{upgrade_name}/{change_name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPONENT_PREFIX: &str = "upgrades/";

    #[test]
    fn keys_should_not_change() {
        insta::assert_snapshot!(change("upgrade_1", "change_1"));
    }

    #[test]
    fn keys_should_have_component_prefix() {
        assert!(change("upgrade_1", "change_1").starts_with(COMPONENT_PREFIX));
    }
}
